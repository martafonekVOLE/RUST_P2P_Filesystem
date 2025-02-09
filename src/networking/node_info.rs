use crate::core::key::Key;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::net::SocketAddr;

///
/// NodeInfo represents a "node contact". Contains the `Key` and `SocketAddr` which are used to
/// uniquely identify and are necessary to communicate with a node.
///
/// Note: In case you send a request to a node knowing only its address, the node will refuse to
/// respond unless you provide its actual `Key`. This is a security measure.
///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NodeInfo {
    pub(crate) id: Key,
    pub(crate) address: SocketAddr,
}

impl NodeInfo {
    pub fn new(id: Key, address: SocketAddr) -> Self {
        NodeInfo { id, address }
    }
}

impl Display for NodeInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} @ {}", self.id, self.address)
    }
}
