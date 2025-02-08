use crate::constants::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use log::error;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;
use tokio::net::unix::SocketAddr;
use uuid::Uuid;

// NodeInfo consists of a Key (which is K bytes) plus the size of the SocketAddr.
const MAX_NODEINFO_SERIALIZED_SIZE: usize = K + size_of::<SocketAddr>();

// Additional overhead for the rest of the message
const OVERHEAD_SIZE: usize = 128;

// For a response that contains up to K NodeInfos, the maximum serialized size would be:
pub const MAX_MESSAGE_SIZE: usize = OVERHEAD_SIZE + K * MAX_NODEINFO_SERIALIZED_SIZE;

pub type RequestId = Uuid;

// TODO document the meanings of the messages
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RequestType {
    Ping,
    FindNode { node_id: Key },
    FindValue { chunk_id: Key },
    Store { chunk_id: Key },
    //
    GetPort { chunk_id: Key },
    // Chunk lookup initiator requests the data upload from the chunk owner node to the given port
    GetValue { chunk_id: Key, port: u16 },
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ResponseType {
    Pong,
    Nodes { node_infos: Vec<NodeInfo> },
    StoreChunkUpdated,
    StoreOK,
    PortOK { port: u16 },
    HasValue { chunk_id: Key },
}

impl Display for ResponseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResponseType::Pong => write!(f, "Pong"),
            ResponseType::Nodes { node_infos: nodes } => {
                write!(f, "Nodes({})", nodes.len())
            }
            ResponseType::StoreChunkUpdated => write!(f, "StoreChunkUpdated"),
            ResponseType::StoreOK => write!(f, "StoreOK"),
            ResponseType::PortOK { port } => write!(f, "PortOK({})", port.to_string()),
            ResponseType::HasValue { chunk_id } => write!(f, "HasValue({})", chunk_id),
        }
    }
}

impl ResponseType {
    pub fn new_nodes(nodes: Vec<NodeInfo>) -> Result<Self, &'static str> {
        if nodes.len() > K {
            Err("The vector exceeds the maximum allowed length")
        } else {
            Ok(ResponseType::Nodes { node_infos: nodes })
        }
    }

    pub fn nodes_into_vec(self) -> Result<Vec<NodeInfo>, &'static str> {
        match self {
            ResponseType::Nodes { node_infos: nodes } => Ok(nodes),
            _ => Err("Attempted to convert wrong response type - expected Nodes"),
        }
    }

    pub fn port_into_u16(self) -> Result<u16, &'static str> {
        match self {
            ResponseType::PortOK { port } => Ok(port),
            _ => Err("Attempted to convert wrong response type - expected Port"),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Request {
    pub(crate) request_type: RequestType,
    pub(crate) sender: NodeInfo,
    pub(crate) receiver: NodeInfo,
    pub(crate) request_id: RequestId,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Response {
    pub(crate) response_type: ResponseType,
    pub(crate) sender: NodeInfo,
    pub(crate) receiver: NodeInfo,
    pub(crate) request_id: RequestId,
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("Invalid message format: {0}")]
    InvalidMessageFormat(String),

    #[error("Unknown request type")]
    UnknownRequestType,

    #[error("Unknown response type")]
    UnknownResponseType,
}

impl Request {
    pub fn new(request_type: RequestType, sender: NodeInfo, receiver: NodeInfo) -> Self {
        Request {
            request_type,
            sender,
            receiver,
            request_id: Uuid::new_v4(),
        }
    }
}

impl Response {
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

    pub fn is_response(&self) -> bool {
        matches!(
            self.response_type,
            ResponseType::Pong | ResponseType::Nodes { .. }
        )
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
