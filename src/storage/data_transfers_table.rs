use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::HashMap;

#[derive(Debug)]
pub struct DataTransfer {
    pub sender: NodeInfo,
    pub chunk_hash: Key,
}

/// Keeps track of chunk transfer requests over TCP.
pub struct DataTransfersTable {
    transfers: HashMap<u16, DataTransfer>, // Maps port to sender info
}

impl Default for DataTransfersTable {
    fn default() -> Self {
        Self::new()
    }
}

impl DataTransfersTable {
    pub fn new() -> DataTransfersTable {
        DataTransfersTable {
            transfers: HashMap::new(),
        }
    }

    pub fn add(&mut self, port: u16, sender: NodeInfo, chunk_hash: Key) -> Option<DataTransfer> {
        self.transfers
            .insert(port, DataTransfer { sender, chunk_hash })
    }

    pub fn get(&mut self, port: u16) -> Option<&DataTransfer> {
        self.transfers.get(&port)
    }

    pub fn remove(&mut self, port: u16) -> Option<DataTransfer> {
        self.transfers.remove(&port)
    }
}
