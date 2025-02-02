use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use crate::sharding::common::Chunk;
use crate::storage::storage_table::StorageTable;
use std::time::SystemTime;

/// Keeps map of uploaded files to reupload them every 24 hours
pub struct FileManager {
    storage_table: StorageTable,
}

impl FileManager {
    pub fn new() -> FileManager {
        FileManager {
            storage_table: StorageTable::new(),
        }
    }

    pub fn check_file_exists(&self, file_path: &str) -> bool {
        // Check if the provided file path exists in the storage path from Config
        todo!()
    }

    pub fn save_data_sent(&self, node: &NodeInfo, hash: Key, saved_at: SystemTime) {
        todo!()
    }
}
