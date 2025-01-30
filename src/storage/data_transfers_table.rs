use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::HashMap;

pub struct DataTransfer {
    sender: NodeInfo,
    key: Key,
}

impl DataTransfer {
    pub fn new(sender: NodeInfo, key: Key) -> DataTransfer {
        DataTransfer { sender, key }
    }

    pub fn get_sender(&self) -> NodeInfo {
        self.sender.clone()
    }

    pub fn get_key(&self) -> Key {
        self.key.clone()
    }
}

pub struct DataTransfersTable {
    transfers: HashMap<u16, DataTransfer>,
}

impl DataTransfersTable {
    pub fn new() -> DataTransfersTable {
        DataTransfersTable {
            transfers: HashMap::new(),
        }
    }

    pub fn add(&mut self, port: u16, sender: NodeInfo, key: Key) -> Option<DataTransfer> {
        self.transfers.insert(port, DataTransfer::new(sender, key))
    }

    pub fn get(&mut self, port: u16) -> Option<&DataTransfer> {
        self.transfers.get(&port)
    }

    pub fn remove(&mut self, port: u16) -> Option<DataTransfer> {
        self.transfers.remove(&port)
    }
}
