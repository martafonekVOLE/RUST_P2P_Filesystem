use crate::constants::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use anyhow::bail;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::mem::size_of;
use std::net::SocketAddr;
use uuid::Uuid;

/// NodeInfo consists of a Key (which is K bytes) plus the size of the SocketAddr.
const MAX_NODEINFO_SERIALIZED_SIZE: usize = K + size_of::<SocketAddr>();

/// Additional overhead for the rest of the message.
const OVERHEAD_SIZE: usize = 128;

/// For a response that contains up to K NodeInfos, the maximum serialized size would be:
pub const MAX_MESSAGE_SIZE: usize = OVERHEAD_SIZE + K * MAX_NODEINFO_SERIALIZED_SIZE;

/// RequestId type is used to identify request-response pairs. Currently, it is just type alias to Uuid.
pub type RequestId = Uuid;

/// Request type enum
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RequestType {
    /// Ping message does check whether it is possible to establish connection with another node.
    Ping,
    /// `FindNode` does try to find the closest nodes to the given node_id.
    FindNode { node_id: Key },
    /// `FindValue` is used to try to retrieve data associated with a given chunk_id.
    FindValue { chunk_id: Key },
    /// `Store` method is used to check whether another node is ready to receive data.
    Store { chunk_id: Key },
    /// `GetPort` does instruct another node to provide a port for TCP data transfer.
    GetPort { chunk_id: Key },
    /// `GetValue` does instruct another node how and where to send data over TCP.
    GetValue { chunk_id: Key, port: u16 },
}

/// Response type enum
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ResponseType {
    /// Pong message is used as reply to incoming Ping message.
    Pong,
    /// Nodes is a response type associated with FindNode request type. It does return a Vector of
    /// nodes which are closer to the requested one.
    Nodes { nodes: Vec<NodeInfo> },
    /// StoreChunkUpdated is a response type associated with Store request type.
    StoreChunkUpdated,
    /// StoreOK is a response type associated with Store request type.
    StoreOK,
    /// PortOK is a response type associated with GetPort request type.
    PortOK { port: u16 },
    /// HasValue is a response type associated with GetValue request type.
    HasValue { chunk_id: Key },
}

impl ResponseType {
    ///
    /// Builder for the Nodes response type. Takes the vector of K nodes and returns and constructs
    /// a new ResponseType::Nodes from them.
    ///
    /// This is used for responding to FindNode requests.
    ///
    pub fn new_nodes(nodes: Vec<NodeInfo>) -> Result<Self> {
        if nodes.len() > K {
            bail!("Nodes response contains more than K nodes");
        } else {
            Ok(ResponseType::Nodes { nodes })
        }
    }
}

///
/// Request struct representing a request message.
///
#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub(crate) request_type: RequestType,
    pub(crate) sender: NodeInfo,
    pub(crate) receiver: NodeInfo,
    pub(crate) request_id: RequestId,
}

impl Request {
    ///
    /// Default constructor for Request struct.
    ///
    pub fn new(request_type: RequestType, sender: NodeInfo, receiver: NodeInfo) -> Self {
        Request {
            request_type,
            sender,
            receiver,
            request_id: Uuid::new_v4(),
        }
    }
}

///
/// Represents a response message.
///
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Response {
    pub(crate) response_type: ResponseType,
    pub(crate) sender: NodeInfo,
    pub(crate) receiver: NodeInfo,
    pub(crate) request_id: RequestId,
}

impl Response {
    ///
    /// Default constructor for Response struct.
    ///
    pub fn new(
        response_type: ResponseType,
        sender: NodeInfo,
        receiver: NodeInfo,
        request_id: Uuid,
    ) -> Self {
        Response {
            response_type,
            sender,
            receiver,
            request_id,
        }
    }
}

impl Display for ResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseType::Pong => write!(f, "Pong"),
            ResponseType::Nodes { nodes } => write!(f, "Nodes({})", nodes.len()),
            ResponseType::StoreChunkUpdated => write!(f, "StoreChunkUpdated"),
            ResponseType::StoreOK => write!(f, "StoreOK"),
            ResponseType::PortOK { port } => write!(f, "PortOK({})", port),
            ResponseType::HasValue { chunk_id } => write!(f, "HasValue({})", chunk_id),
        }
    }
}

impl Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestType::Ping => write!(f, "Ping"),
            RequestType::FindNode { node_id } => write!(f, "FindNode({})", node_id),
            RequestType::FindValue { chunk_id } => write!(f, "FindValue({})", chunk_id),
            RequestType::Store { chunk_id } => write!(f, "Store({})", chunk_id),
            RequestType::GetPort { chunk_id } => write!(f, "GetPort({})", chunk_id),
            RequestType::GetValue { chunk_id, port } => {
                write!(f, "GetValue({}, {})", chunk_id, port)
            }
        }
    }
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {} -> {}",
            self.response_type, self.request_id, self.sender.id, self.receiver.id
        )
    }
}

impl Display for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {} -> {}",
            self.request_type, self.request_id, self.sender.id, self.receiver.id
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    use std::net::SocketAddr;

    /// Helper to create a dummy NodeInfo.
    /// The node gets a random key via `Key::new_random()` and an address derived from the given port.
    fn dummy_node_info(port: u16) -> NodeInfo {
        let key = Key::new_random();
        let address: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        NodeInfo::new(key, address)
    }

    /// Test that `Request::new` sets the fields correctly and that its Display impl
    /// contains the expected substrings.
    #[test]
    fn test_request_new_and_display() {
        let sender = dummy_node_info(1000);
        let receiver = dummy_node_info(2000);
        let request_type = RequestType::Ping;
        let request = Request::new(request_type.clone(), sender.clone(), receiver.clone());

        let display_str = format!("{}", request);
        // Check that the display output contains "Ping" and the sender/receiver keys.
        let sender_id_str = format!("{}", sender.id);
        let receiver_id_str = format!("{}", receiver.id);
        assert!(display_str.contains("Ping"));
        assert!(display_str.contains(&sender_id_str));
        assert!(display_str.contains(&receiver_id_str));
    }

    /// Test that the builder for a Nodes response (ResponseType::new_nodes) works as expected.
    #[test]
    fn test_new_nodes_builder() {
        let k = K;
        let mut nodes = Vec::with_capacity(k);
        for i in 0..k {
            // Use different ports so that each NodeInfo is unique.
            nodes.push(dummy_node_info(7000 + i as u16));
        }
        let result = ResponseType::new_nodes(nodes);
        assert!(result.is_ok());

        // Now create a vector with K + 1 nodes and ensure it errors.
        let mut nodes_exceed = Vec::with_capacity(k + 1);
        for i in 0..(k + 1) {
            nodes_exceed.push(dummy_node_info(8000 + i as u16));
        }
        let result_err = ResponseType::new_nodes(nodes_exceed);
        assert!(result_err.is_err());
    }

    /// Test Display implementations for RequestType variants.
    #[test]
    fn test_display_request_type() {
        let key = Key::new_random();
        let rt_ping = RequestType::Ping;
        let rt_find_node = RequestType::FindNode { node_id: key };
        let rt_find_value = RequestType::FindValue { chunk_id: key };
        let rt_store = RequestType::Store { chunk_id: key };
        let rt_get_port = RequestType::GetPort { chunk_id: key };
        let rt_get_value = RequestType::GetValue {
            chunk_id: key,
            port: 1234,
        };

        assert!(format!("{}", rt_ping).contains("Ping"));
        assert!(format!("{}", rt_find_node).contains("FindNode"));
        assert!(format!("{}", rt_find_value).contains("FindValue"));
        assert!(format!("{}", rt_store).contains("Store("));
        assert!(format!("{}", rt_get_port).contains("GetPort("));
        assert!(format!("{}", rt_get_value).contains("GetValue"));
    }

    /// Test Display implementations for ResponseType variants.
    #[test]
    fn test_display_response_type() {
        let key = Key::new_random();
        let rt_pong = ResponseType::Pong;
        let rt_nodes = ResponseType::Nodes {
            nodes: vec![dummy_node_info(9000), dummy_node_info(9001)],
        };
        let rt_store_chunk_updated = ResponseType::StoreChunkUpdated;
        let rt_store_ok = ResponseType::StoreOK;
        let rt_port_ok = ResponseType::PortOK { port: 4321 };
        let rt_has_value = ResponseType::HasValue { chunk_id: key };

        assert!(format!("{}", rt_pong).contains("Pong"));
        assert!(format!("{}", rt_nodes).contains("Nodes("));
        assert!(format!("{}", rt_store_chunk_updated).contains("StoreChunkUpdated"));
        assert!(format!("{}", rt_store_ok).contains("StoreOK"));
        assert!(format!("{}", rt_port_ok).contains("PortOK"));
        assert!(format!("{}", rt_has_value).contains("HasValue"));
    }

    /// Test that MAX_MESSAGE_SIZE is computed as expected.
    #[test]
    fn test_max_message_size() {
        let expected = OVERHEAD_SIZE + K * (K + size_of::<SocketAddr>());
        assert_eq!(MAX_MESSAGE_SIZE, expected);
    }
}
