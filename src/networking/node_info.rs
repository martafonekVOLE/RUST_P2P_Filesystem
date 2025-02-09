use crate::core::key::Key;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::net::SocketAddr;

///
/// NodeInfo struct contains `Key` and `SocketAddr` of a node in the network. This is used as a contact
/// for the node.
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NodeInfo {
    pub(crate) id: Key,
    pub(crate) address: SocketAddr,
}

impl NodeInfo {
    ///
    /// Default constructor.
    ///
    pub fn new(id: Key, address: SocketAddr) -> Self {
        NodeInfo { id, address }
    }
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.id, self.address)
    }
}
