//! The unsafe module. Each block states why it is sound. Handles are owned by `OwnedHandle` (closed on drop); memory that Windows
//! allocates (`LocalFree`) is copied out and freed before the function returns.
use std::ffi::c_void;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    ConvertStringSidToSidW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    GetLengthSid, GetTokenInformation, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenSessionId, TokenUser,
};
use windows_sys::Win32::System::LibraryLoader::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, SetDefaultDllDirectories,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

fn last() -> io::Error {
    io::Error::last_os_error()
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

/// A user's security identifier: its binary form (compared as bytes, which is what `EqualSid` does) and its `S-1-5-21-...` text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sid {
    bytes: Vec<u8>,
    text: String,
}

impl Sid {
    /// The `S-1-...` text form, which is safe in a pipe name and in an SDDL string.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// A SID from its text form (tests use it to stand in for another user).
    pub fn from_string(text: &str) -> io::Result<Sid> {
        let w = wide(text);
        let mut psid: *mut c_void = std::ptr::null_mut();
        // SAFETY: `w` is a NUL-terminated UTF-16 string that outlives the call, and `psid` is a valid out pointer.
        if unsafe { ConvertStringSidToSidW(w.as_ptr(), &mut psid) } == 0 {
            return Err(last());
        }
        // SAFETY: on success `psid` points to a valid SID that Windows allocated with LocalAlloc.
        let s = unsafe { Self::copy(psid) };
        // SAFETY: `psid` came from LocalAlloc above and is not used again.
        unsafe { LocalFree(psid) };
        s
    }

    /// Copies the SID at `psid`.
    ///
    /// # Safety
    /// `psid` must point to a valid SID.
    unsafe fn copy(psid: *mut c_void) -> io::Result<Sid> {
        // SAFETY: the caller guarantees a valid SID; GetLengthSid reads its header, and the slice covers exactly that length.
        let bytes = unsafe {
            let n = GetLengthSid(psid) as usize;
            std::slice::from_raw_parts(psid as *const u8, n).to_vec()
        };
        let mut p: *mut u16 = std::ptr::null_mut();
        // SAFETY: valid SID in, valid out pointer.
        if unsafe { ConvertSidToStringSidW(psid, &mut p) } == 0 {
            return Err(last());
        }
        // SAFETY: on success `p` is a NUL-terminated UTF-16 string allocated by Windows; read to the NUL, then free it once.
        let text = unsafe {
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            let t = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            LocalFree(p as *mut c_void);
            t
        };
        Ok(Sid { bytes, text })
    }

    /// The user of this process, and the Windows session it runs in (both go into the pipe name).
    pub fn current() -> io::Result<(Sid, u32)> {
        // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing.
        let tok = process_token(unsafe { GetCurrentProcess() })?;
        let h = tok.as_raw_handle();
        let session = token_info(h, TokenSessionId)?[0] as u32;
        Ok((token_user(h)?, session))
    }
}

/// A token information block, `u64`-aligned for the pointer inside `TOKEN_USER`.
fn token_info(token: HANDLE, class: i32) -> io::Result<Vec<u64>> {
    let mut len = 0u32;
    // SAFETY: a null buffer of length 0 asks only for the size; `len` is a valid out pointer. It fails with INSUFFICIENT_BUFFER.
    unsafe { GetTokenInformation(token, class, std::ptr::null_mut(), 0, &mut len) };
    let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
    // SAFETY: `buf` has at least `len` bytes (rounded up to 8) and is 8-aligned; `token` is a valid token handle.
    if unsafe { GetTokenInformation(token, class, buf.as_mut_ptr().cast(), len, &mut len) } == 0 {
        return Err(last());
    }
    Ok(buf)
}

fn token_user(token: HANDLE) -> io::Result<Sid> {
    let buf = token_info(token, TokenUser)?;
    // SAFETY: the buffer holds a TOKEN_USER (8-aligned, filled by Windows) whose `Sid` points inside that same buffer.
    unsafe { Sid::copy((*(buf.as_ptr() as *const TOKEN_USER)).User.Sid) }
}

fn process_token(process: HANDLE) -> io::Result<OwnedHandle> {
    let mut t: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` is a valid process handle and `t` a valid out pointer.
    if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut t) } == 0 {
        return Err(last());
    }
    // SAFETY: OpenProcessToken succeeded, so `t` is a new handle that nothing else owns.
    Ok(unsafe { OwnedHandle::from_raw_handle(t) })
}

/// Who runs the process that owns the server end of the named pipe that the client handle `pipe` is connected to. Any failure (a
/// process of another user cannot be opened with these rights) is an error, and the caller must treat it as "not ours".
pub fn pipe_server_sid(pipe: &impl AsRawHandle) -> io::Result<Sid> {
    let mut pid = 0u32;
    // SAFETY: `pipe` is a valid open handle for the call; `pid` is a valid out pointer.
    if unsafe { GetNamedPipeServerProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(last());
    }
    // SAFETY: plain call; a null result is checked below.
    let p = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if p.is_null() {
        return Err(last());
    }
    // SAFETY: `p` is a new process handle that nothing else owns.
    let p = unsafe { OwnedHandle::from_raw_handle(p) };
    token_user(process_token(p.as_raw_handle())?.as_raw_handle())
}

/// Creates one instance of a named pipe whose DACL grants access to `owner` only (`D:P(A;;GA;;;<sid>)`: protected, nobody else, not
/// even read access). The handle is not inheritable. Must run inside a tokio runtime context.
pub fn create_user_only_pipe(
    opts: &mut ServerOptions,
    name: &str,
    owner: &Sid,
) -> io::Result<NamedPipeServer> {
    let sddl = wide(&format!("D:P(A;;GA;;;{})", owner.as_str()));
    let mut sd: *mut c_void = std::ptr::null_mut();
    // SAFETY: `sddl` is NUL-terminated and outlives the call; `sd` is a valid out pointer.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            SDDL_REVISION_1,
            &mut sd,
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(last());
    }
    let mut sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: sd,
        bInheritHandle: 0,
    };
    // SAFETY: `sa` is a valid SECURITY_ATTRIBUTES whose descriptor `sd` stays allocated until after the call; CreateNamedPipe copies it.
    let r = unsafe {
        opts.create_with_security_attributes_raw(name, (&mut sa as *mut SECURITY_ATTRIBUTES).cast())
    };
    // SAFETY: `sd` came from the conversion above and is not used again.
    unsafe { LocalFree(sd) };
    r
}

/// Restricts the DLL search of this process to the application directory, System32 and the directories added explicitly: not the
/// current directory, not `PATH`, for later `LoadLibrary` calls. (Static imports resolve before `main`; see the msvc link flag.)
pub fn set_default_dll_directories() -> io::Result<()> {
    // SAFETY: a plain call with a flag constant.
    if unsafe { SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_DEFAULT_DIRS) } == 0 {
        return Err(last());
    }
    Ok(())
}
