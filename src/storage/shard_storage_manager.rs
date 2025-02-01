use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use crate::storage::data_transfers_table::{DataTransfer, DataTransfersTable};
use crate::storage::file_manager::Chunk;
use crate::storage::storage_table::StorageTable;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, RwLock};

pub struct ShardStorageManager {
    filepath: String,
    storage_table: StorageTable,
    data_transfers_table: DataTransfersTable,
}

impl ShardStorageManager {
    pub fn new(filepath: String) -> ShardStorageManager {
        ShardStorageManager {
            filepath,
            storage_table: StorageTable::new(),
            data_transfers_table: DataTransfersTable::new(),
        }
    }

    pub fn save_for_port(&mut self, data: Vec<u8>, port: u16) -> Result<(), &str> {
        let data_transfer = self.data_transfers_table.get(port);
        let filepath = self.filepath.clone();

        match data_transfer {
            // Future-improvement: we may use
            Some(data_transfer) => {
                let mut file =
                    File::create(filepath + "/" + data_transfer.get_key().to_string().as_str())
                        .expect("Can't create file.");

                self.data_transfers_table.remove(port);

                file.write_all(&*data).expect("Failed to save file.");
            }
            None => {
                return Err("Unable to save file.");
            }
        };

        Ok(())
    }

    pub fn add_active_tcp_connection(
        &mut self,
        port: u16,
        sender: NodeInfo,
        key: Key,
    ) -> Option<DataTransfer> {
        self.data_transfers_table.add(port, sender, key)
    }

    pub fn get_data_transfers_table(&self) -> &DataTransfersTable {
        &self.data_transfers_table
    }
}
