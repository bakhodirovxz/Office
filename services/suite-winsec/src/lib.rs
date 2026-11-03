//! Win32 security helpers for `suite-desktop`, safe to call. All `unsafe` is in `sys.rs`.
#![cfg(windows)]

#[allow(unsafe_code)]
mod sys;
pub use sys::{Ace, Sid, create_user_only_pipe, pipe_server_sid, set_default_dll_directories};

/// Is the folder `path` safe to trust as machine-wide (admin-written) data? `Err` carries the reason.
pub fn folder_is_admin_only(path: &std::path::Path) -> Result<(), String> {
    let (owner, dacl) =
        sys::folder_security(path).map_err(|e| format!("cannot read its permissions: {e}"))?;
    verdict(&owner, dacl.as_deref())
}

/// SYSTEM, Administrators.
const OWNERS: [&str; 2] = ["S-1-5-18", "S-1-5-32-544"];
/// Write, append, EA, attributes, delete-child, delete, write-DAC, write-owner, generic write and generic all.
const WRITE: u32 =
    0x2 | 0x4 | 0x10 | 0x40 | 0x100 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x4000_0000 | 0x1000_0000;

/// The decision, on plain data: the owner must be SYSTEM or Administrators, and nobody else (TrustedInstaller aside) may hold a
/// write right. A NULL DACL (everybody) is unsafe.
pub fn verdict(owner: &str, dacl: Option<&[Ace]>) -> Result<(), String> {
    if !OWNERS.contains(&owner) {
        return Err(format!(
            "it is owned by {owner}, not by Administrators or SYSTEM"
        ));
    }
    let Some(aces) = dacl else {
        return Err("it has no access list (everybody may write)".into());
    };
    match aces.iter().find(|a| {
        a.mask & WRITE != 0 && !OWNERS.contains(&a.sid.as_str()) && !a.sid.starts_with("S-1-5-80-")
    }) {
        Some(a) => Err(format!("{} may write to it", a.sid)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ace(sid: &str, mask: u32) -> Ace {
        Ace {
            sid: sid.into(),
            mask,
        }
    }

    #[test]
    fn the_verdict_needs_an_admin_owner_and_no_other_writer() {
        let read = 0x1200a9; // read and execute
        let good = [
            ace("S-1-5-18", 0x1f01ff),
            ace("S-1-5-32-544", 0x1f01ff),
            ace("S-1-5-32-545", read),
        ];
        assert!(verdict("S-1-5-32-544", Some(&good)).is_ok());
        assert!(verdict("S-1-5-18", Some(&good)).is_ok());
        assert!(
            verdict("S-1-5-21-1-2-3-1001", Some(&good)).is_err(),
            "a user owns it"
        );
        assert!(verdict("S-1-5-32-544", None).is_err(), "NULL DACL");
        for sid in ["S-1-5-32-545", "S-1-5-11", "S-1-1-0", "S-1-5-21-1-2-3-1001"] {
            for mask in [0x2, 0x4, 0x40, 0x1000_0000, 0x4000_0000, 0x1f01ff] {
                let mut v = good.to_vec();
                v.push(ace(sid, mask));
                assert!(verdict("S-1-5-32-544", Some(&v)).is_err(), "{sid} {mask:x}");
            }
        }
    }

    #[test]
    fn a_folder_made_by_a_user_is_refused() {
        let d = std::env::temp_dir().join(format!("winsec-dir-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (owner, dacl) = sys::folder_security(&d).unwrap();
        assert!(dacl.is_some_and(|a| !a.is_empty()));
        assert!(owner.starts_with("S-1-5-"));
        let e = folder_is_admin_only(&d).unwrap_err();
        std::fs::remove_dir_all(&d).unwrap();
        assert!(e.contains("owned by") || e.contains("may write"), "{e}");
    }
}
