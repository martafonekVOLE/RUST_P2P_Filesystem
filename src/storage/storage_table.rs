use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use crate::core::key::Key;
use crate::storage::file_info::FileInfo;

pub struct StorageTable {
    storage: HashMap<Key, FileInfo>,
}

///
/// Data storage table
///
impl StorageTable {
    pub fn new() -> Self {
        StorageTable {
            storage: HashMap::new(),
        }
    }

    pub fn get(&self, key: &Key) -> Option<&FileInfo> {
        self.storage.get(key)
    }

    pub fn add(&mut self, key: Key, file_info: FileInfo) {
        if let Some(_) = self.get(&key) {
            self.remove(&key);
        }

        self.storage.insert(key, file_info);
    }

    pub fn remove(&mut self, key: &Key) {
        self.storage.remove(key);
    }
}
