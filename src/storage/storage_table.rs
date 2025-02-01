use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::HashMap;
use std::time::SystemTime;

pub struct FileInfo {
    pub time: SystemTime,
    pub ttl: u16,
    pub uploader: NodeInfo,
}

pub struct StorageTable {
    pub storage: HashMap<Key, FileInfo>,
}

impl StorageTable {
    pub fn new() -> StorageTable {
        StorageTable {
            storage: HashMap::new(),
        }
    }
}
