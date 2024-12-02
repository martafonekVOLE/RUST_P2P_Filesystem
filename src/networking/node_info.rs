use crate::core::key::Key;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: Key,
    pub address: SocketAddr,
}

impl NodeInfo {
    pub fn new(id: Key, address: SocketAddr) -> Self {
        NodeInfo { id, address }
    }

    pub fn get_address(&self) -> &SocketAddr {
        &self.address
    }

    pub fn get_id(&self) -> Key {
        self.id
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(self.id.to_bytes());
        bytes.extend(self.address.to_string().as_bytes());

        bytes
    }
}
