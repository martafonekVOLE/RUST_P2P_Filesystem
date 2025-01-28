use crate::core::key::Key;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

    pub fn create_local_node() -> Self {
        let id = Key::new_random();
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("Failed to bind to address");
        let address = socket.local_addr().expect("Failed to get local address");
        NodeInfo { id, address }
    }
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.id, self.address)
    }
}
