use std::sync::Arc;
use tokio::sync::RwLock;
use crate::storage::storage_table::StorageTable;

pub struct StorageManager {
    storage_path: String,
    storage_table: Arc<RwLock<StorageTable>>,
}

impl StorageManager {
    pub fn new(storage_path: String) -> StorageManager {
        StorageManager{
            storage_path,
            storage_table: Arc::new(RwLock::new(StorageTable::new()))
        }
    }

    pub fn store_data() {
        todo!()
    }
}