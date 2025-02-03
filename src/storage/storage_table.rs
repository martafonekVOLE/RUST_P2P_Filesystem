use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::HashMap;
use std::time::SystemTime;

// TODO: Maybe rename to ChunkFileInfo or ShardFileInfo?
pub struct FileInfo {
    pub time: SystemTime, // Time when chunk was saved
    pub ttl: u16,         // to know when to republish the file
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
