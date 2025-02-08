use crate::constants::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

use anyhow::Result;
use tokio::net::unix::SocketAddr;

/// NodeInfo consists of a Key (which is K bytes) plus the size of the SocketAddr.
const MAX_NODEINFO_SERIALIZED_SIZE: usize = K + size_of::<SocketAddr>();

/// Additional overhead for the rest of the message
const OVERHEAD_SIZE: usize = 128;

/// For a response that contains up to K NodeInfos, the maximum serialized size would be:
pub const MAX_MESSAGE_SIZE: usize = OVERHEAD_SIZE + K * MAX_NODEINFO_SERIALIZED_SIZE;

///
/// RequestId type is used to identify request-response pairs.
///
/// Currently, it is just type alias to Uuid.
///
pub type RequestId = Uuid;

///
/// Request type enum
///
/// This does represent different types of requests which are used in the system.
///
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RequestType {
    /// Ping message does check whether it is possible to establish connection with another node
    Ping,
    /// `FindNode` does try to find the closest nodes to the given node_id
    FindNode { node_id: Key },
    /// `FindValue` is used to try to retrieve data associated with a given chunk_id
    FindValue { chunk_id: Key },
    /// `Store` method is used to check whether another node is ready to receive data
    Store { chunk_id: Key },
    /// `GetPort` does instruct another node to provide a port for TCP data transfer
    GetPort { chunk_id: Key },
    /// `GetValue` does instruct another node how and where to send data over TCP
    GetValue { chunk_id: Key, port: u16 },
}

///
/// Response type enum
///
/// This does represent different types of responses which are used in the system.
///
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ResponseType {
    /// Pong message is used as reply to incoming Ping message. This way the Pong receiver may verify,
    /// that sender is available
    Pong,
    /// Nodes is a response type associated with FindNode request type. It does return a Vector of
    /// nodes which are closer to the requested one.
    Nodes { nodes: Vec<NodeInfo> },
    /// StoreChunkUpdated is a response type associated with Store request type. It does notify the
    /// receiver that the chunk he tried to store is already present and its TTS has been updated.
    StoreChunkUpdated,
    /// StoreOK is a response type associated with Store request type. It does notify the receiver
    /// that sender is available a ready to receive data.
    StoreOK,
    /// PortOK is a response type associated with GetPort request type. It does provide receiver with
    /// a port, where the sender is waiting for the TCP data transfer.
    PortOK { port: u16 },
    /// HasValue is a response type associated with GetValue request type. It does inform the receiver
    /// that the sender does have the requested value and is able to send it to the receiver.
    HasValue { chunk_id: Key },
}

impl ResponseType {
    ///
    /// Builder for Nodes response type
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
/// Request struct
///
/// This struct does represent a request. Sender and Receiver are represented
/// as NodeInfo struct. This struct does provide additional information about
/// each node. RequestId does specify an ID of request and RequestType does
/// specify a "message type" of the request.
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
    /// Default constructor for Request
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
/// Response struct
///
/// This struct does represent a response. Sender and Receiver are represented
/// as `NodeInfo` struct. This struct does provide additional information about
/// each node. `RequestId` does specify an ID of the original request and `ResponseType`
/// does specify a "message type" of the response.
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
    /// Method for creating a new Response
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

    ///
    /// Method which does convert Response into bytes
    ///
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize response")
    }

    ///
    /// Determines if the `request` if a response
    ///
    pub fn is_response(&self) -> bool {
        matches!(
            self.response_type,
            ResponseType::Pong
                | ResponseType::Nodes { .. }
                | ResponseType::StoreOK
                | ResponseType::PortOK { .. }
                | ResponseType::HasValue { .. }
        )
    }
}

impl Display for ResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseType::Pong => write!(f, "Pong"),
            ResponseType::Nodes { nodes } => {
                write!(f, "Nodes({})", nodes.len())
            }
            ResponseType::StoreChunkUpdated => write!(f, "StoreChunkUpdated"),
            ResponseType::StoreOK => write!(f, "StoreOK"),
            ResponseType::PortOK { port } => write!(f, "PortOK({})", port.to_string()),
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
            RequestType::Store { chunk_id: file_id } => write!(f, "Store({})", file_id),
            RequestType::GetPort { chunk_id: file_id } => write!(f, "GetPort({})", file_id),
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
    ///
    /// Method used for formatting
    ///
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} [{}] {} -> {}",
            self.request_type, self.request_id, self.sender.id, self.receiver.id
        )
    }
}
