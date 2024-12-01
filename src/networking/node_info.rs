use crate::core::key::Key;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::process::exit;

#[derive(Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: Key,
    pub address: Option<SocketAddr>,
}

impl NodeInfo {
    pub fn new(id: Key, address: Option<SocketAddr>) -> Self {
        NodeInfo { id, address }
    }

    pub fn get_address(&self) -> Option<SocketAddr> {
        self.address
    }

    pub fn get_id(&self) -> Key {
        self.id
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        bytes.extend(self.id.to_bytes());

        if let Some(addr) = &self.address {
            // Assuming SocketAddr can be serialized directly to bytes
            bytes.extend(addr.to_string().as_bytes());
        } else {
            bytes.extend(&[0u8]);
        }

        bytes
    }
}
