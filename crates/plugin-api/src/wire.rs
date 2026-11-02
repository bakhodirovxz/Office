//! The message envelope (PLAN phase 7, section 4.2): `u32 header_len, header (UTF-8 JSON), u32 blob_count, per blob u32 len, bytes`.
//! All integers are little endian. The reader never allocates from a length it has not checked against the bytes left.

use std::fmt;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Input and output of one round, each.
pub const MAX_MESSAGE: usize = 32 << 20;
pub const MAX_HEADER: usize = 1 << 20;
pub const MAX_BLOBS: usize = 16;
pub const MAX_ROUNDS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireError {
    Truncated,
    TooLarge,
    HeaderTooLarge,
    TooManyBlobs,
    BadUtf8,
    Trailing,
    Json(String),
    /// A text map blob that breaks a rule (the string names it).
    Textmap(&'static str),
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => f.write_str("message is truncated"),
            WireError::TooLarge => f.write_str("message is over 32 MiB"),
            WireError::HeaderTooLarge => f.write_str("header is over 1 MiB"),
            WireError::TooManyBlobs => f.write_str("more than 16 blobs"),
            WireError::BadUtf8 => f.write_str("text is not valid UTF-8"),
            WireError::Trailing => f.write_str("bytes after the end of the message"),
            WireError::Json(m) => write!(f, "header JSON: {m}"),
            WireError::Textmap(m) => write!(f, "text map: {m}"),
        }
    }
}

impl std::error::Error for WireError {}

/// A bounds-checked cursor.
pub(crate) struct Rd<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Rd<'a> {
    pub(crate) fn new(b: &'a [u8]) -> Self {
        Rd { b, i: 0 }
    }
    pub(crate) fn left(&self) -> usize {
        self.b.len() - self.i
    }
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if n > self.left() {
            return Err(WireError::Truncated);
        }
        let s = &self.b[self.i..self.i + n];
        self.i += n;
        Ok(s)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("2 bytes"),
        ))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    /// A count of items that each take at least `min` bytes: a count the bytes left cannot hold is truncation.
    pub(crate) fn count(&mut self, min: usize) -> Result<usize, WireError> {
        let n = self.u32()? as usize;
        if n > self.left() / min {
            return Err(WireError::Truncated);
        }
        Ok(n)
    }
    pub(crate) fn str(&mut self) -> Result<String, WireError> {
        let n = self.u32()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| WireError::BadUtf8)
    }
    pub(crate) fn finish(self) -> Result<(), WireError> {
        if self.left() == 0 {
            Ok(())
        } else {
            Err(WireError::Trailing)
        }
    }
}

pub(crate) fn put_u32(o: &mut Vec<u8>, v: usize) -> Result<(), WireError> {
    o.extend_from_slice(
        &u32::try_from(v)
            .map_err(|_| WireError::TooLarge)?
            .to_le_bytes(),
    );
    Ok(())
}

pub(crate) fn put_str(o: &mut Vec<u8>, s: &str) -> Result<(), WireError> {
    put_u32(o, s.len())?;
    o.extend_from_slice(s.as_bytes());
    Ok(())
}

/// One call or one answer: a JSON header and the blobs it points to by index.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Message {
    pub header: String,
    pub blobs: Vec<Vec<u8>>,
}

impl Message {
    pub fn new<T: Serialize>(header: &T, blobs: Vec<Vec<u8>>) -> Result<Self, WireError> {
        Ok(Message {
            header: serde_json::to_string(header).map_err(|e| WireError::Json(e.to_string()))?,
            blobs,
        })
    }

    /// The header as a typed value (plugins ignore unknown fields in a host call; the host does not in an answer: see `action`).
    pub fn header_as<T: DeserializeOwned>(&self) -> Result<T, WireError> {
        serde_json::from_str(&self.header).map_err(|e| WireError::Json(e.to_string()))
    }

    pub fn encode(&self) -> Result<Vec<u8>, WireError> {
        if self.header.len() > MAX_HEADER {
            return Err(WireError::HeaderTooLarge);
        }
        if self.blobs.len() > MAX_BLOBS {
            return Err(WireError::TooManyBlobs);
        }
        let total = 8 + self.header.len() + self.blobs.iter().map(|b| 4 + b.len()).sum::<usize>();
        if total > MAX_MESSAGE {
            return Err(WireError::TooLarge);
        }
        let mut o = Vec::with_capacity(total);
        put_str(&mut o, &self.header)?;
        put_u32(&mut o, self.blobs.len())?;
        for b in &self.blobs {
            put_u32(&mut o, b.len())?;
            o.extend_from_slice(b);
        }
        Ok(o)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() > MAX_MESSAGE {
            return Err(WireError::TooLarge);
        }
        let mut r = Rd::new(bytes);
        let hl = r.u32()? as usize;
        if hl > MAX_HEADER {
            return Err(WireError::HeaderTooLarge);
        }
        let header = String::from_utf8(r.take(hl)?.to_vec()).map_err(|_| WireError::BadUtf8)?;
        let n = r.u32()? as usize;
        if n > MAX_BLOBS {
            return Err(WireError::TooManyBlobs);
        }
        let mut blobs = Vec::with_capacity(n);
        for _ in 0..n {
            let len = r.u32()? as usize;
            blobs.push(r.take(len)?.to_vec());
        }
        r.finish()?;
        Ok(Message { header, blobs })
    }
}
