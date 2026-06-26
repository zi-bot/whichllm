use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: u64,
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new() -> Option<Self> {
        let base = dirs::cache_dir()?;
        let dir = base.join("whichllm");
        std::fs::create_dir_all(&dir).ok()?;
        Some(Self { dir })
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str, ttl: Duration) -> Option<T> {
        let path = self.dir.join(format!("{key}.json"));
        let raw = std::fs::read_to_string(&path).ok()?;
        let entry: CacheEntry<T> = serde_json::from_str(&raw).ok()?;
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();
        if now.saturating_sub(entry.timestamp) > ttl.as_secs() {
            return None;
        }
        Some(entry.data)
    }

    pub fn set<T: Serialize>(&self, key: &str, data: &T) {
        let path = self.dir.join(format!("{key}.json"));
        let entry = CacheEntry {
            data,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };
        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = std::fs::write(&path, json);
        }
    }
}
