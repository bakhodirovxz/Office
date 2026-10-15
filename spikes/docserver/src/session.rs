//! In-memory sessions; document versions are files under `<data_dir>/<sid>/<n>.docx`.
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Session {
    pub name: String,
    pub mode: String,
    pub readonly: bool,
    pub key: String,
    pub callback_url: String,
    pub filetype: String,
    pub user_id: String,
    /// Highest stored version (0 = the document as fetched from the HRM).
    pub last_version: u32,
}

#[derive(Default)]
pub struct Sessions {
    map: HashMap<String, Session>,
}

impl Sessions {
    pub fn insert(&mut self, sid: String, s: Session) {
        self.map.insert(sid, s);
    }
    pub fn get(&self, sid: &str) -> Option<Session> {
        self.map.get(sid).cloned()
    }
    /// Reserve the next version number.
    pub fn next_version(&mut self, sid: &str) -> Option<u32> {
        let s = self.map.get_mut(sid)?;
        s.last_version += 1;
        Some(s.last_version)
    }
}

pub fn version_path(data_dir: &std::path::Path, sid: &str, ver: u32) -> PathBuf {
    data_dir.join(sid).join(format!("{ver}.docx"))
}
