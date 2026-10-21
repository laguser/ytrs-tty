use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub duration: String,
    pub timestamp: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub struct AppStorage {
    pub history: Vec<HistoryEntry>,
    pub bookmarks: Vec<HistoryEntry>,
}

impl AppStorage {
    fn path() -> PathBuf {
        let mut p = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("/home/lag/.local/share"));
        p.push("yt");
        let _ = fs::create_dir_all(&p);
        p.push("storage.json");
        p
    }

    pub fn load() -> Self {
        let p = Self::path();
        if let Ok(data) = fs::read_to_string(&p) {
            if let Ok(st) = serde_json::from_str(&data) {
                return st;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Ok(data) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, data);
        }
    }

    pub fn add_history(&mut self, entry: HistoryEntry) {
        // Remove existing duplicate
        self.history.retain(|e| e.id != entry.id);
        self.history.insert(0, entry);
        if self.history.len() > 100 {
            self.history.truncate(100);
        }
        self.save();
    }

    pub fn toggle_bookmark(&mut self, entry: HistoryEntry) -> bool {
        if let Some(pos) = self.bookmarks.iter().position(|e| e.id == entry.id) {
            self.bookmarks.remove(pos);
            self.save();
            false
        } else {
            self.bookmarks.insert(0, entry);
            self.save();
            true
        }
    }
}
