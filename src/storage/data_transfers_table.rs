use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::HashMap;

pub struct DataTransfer {
    pub sender: NodeInfo,
    pub chunk_hash: Key,
}

// TOOD: Make members public instead of adding getters?
// impl DataTransfer {
//     pub fn new(sender: NodeInfo, key: Key) -> DataTransfer {
//         DataTransfer {
//             sender,
//             chunk_hash: key,
//         }
//     }

//     pub fn get_sender(&self) -> NodeInfo {
//         self.sender.clone()
//     }

//     pub fn get_key(&self) -> Key {
//         self.chunk_hash.clone()
//     }
// }

pub struct DataTransfersTable {
    transfers: HashMap<u16, DataTransfer>, // Maps port to sender info
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
