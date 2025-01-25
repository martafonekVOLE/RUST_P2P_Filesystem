use std::time::{Duration, SystemTime};

pub struct FileInfo {
    filepath: String,
    time: SystemTime,
    ttl: Duration,
}

impl FileInfo {
    pub fn new(filepath: String, ttl: Duration) -> Self {
        FileInfo {
            filepath,
            time: SystemTime::now(),
            ttl
        }
    }
}
