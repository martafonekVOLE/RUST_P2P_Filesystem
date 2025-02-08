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

    pub fn get_id(&self) -> Key {
        self.id
    } // TODO Change all usages to direct id access
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.id, self.address)
    }
}
