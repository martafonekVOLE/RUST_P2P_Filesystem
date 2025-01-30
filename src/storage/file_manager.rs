use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use crate::storage::storage_table::StorageTable;
use std::time::SystemTime;

#[derive(Clone)]
pub struct Chunk {
    hash: Key,
    data: Vec<u8>,
}

impl Chunk {
    pub fn get_hash(&self) -> Key {
        self.hash.clone()
    }
    pub fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }
}

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

    pub fn temp_sharding() -> Option<Chunk> {
        Some(Chunk {
            hash: Key::from_input(String::new().as_bytes()),
            data: Vec::new(),
        })
    }

    pub fn save_data_sent(&self, node: &NodeInfo, hash: Key, saved_at: SystemTime) {
        todo!()
    }
}
