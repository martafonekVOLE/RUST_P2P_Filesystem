use crate::constants::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use thiserror::Error;
use uuid::Uuid;

// TODO docstrings

pub type RequestId = Uuid; // TODO make a custom strut, abstract away implementation?

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum RequestType {
    Ping,
    FindNode { node_id: Key },
    FindValue { chunk_id: Key },
    Store { file_id: Key },
    GetPort { file_id: Key },
    GetValue { chunk_id: Key, port: u16 },
}

impl Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestType::Ping => write!(f, "Ping"),
            RequestType::FindNode { node_id } => write!(f, "FindNode({})", node_id),
            RequestType::FindValue { chunk_id } => write!(f, "FindValue({})", chunk_id),
            RequestType::Store { file_id } => write!(f, "Store({})", file_id),
            RequestType::GetPort { file_id } => write!(f, "GetPort({})", file_id),
            RequestType::GetValue { chunk_id, port } => {
                write!(f, "GetValue({}, {})", chunk_id, port)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum ResponseType {
    Pong,
    Nodes { nodes: Vec<NodeInfo> },
    StoreChunkUpdated,
    StoreOK,
    PortOK { port: u16 },
    HasValue { chunk_id: Key },
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

impl ResponseType {
    pub fn new_nodes(nodes: Vec<NodeInfo>) -> Result<Self, &'static str> {
        if nodes.len() > K {
            Err("The vector exceeds the maximum allowed length")
        } else {
            Ok(ResponseType::Nodes { nodes })
        }
    }

    pub fn nodes_into_vec(self) -> Result<Vec<NodeInfo>, &'static str> {
        match self {
            ResponseType::Nodes { nodes } => Ok(nodes),
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

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|e| {
            log::error!("Failed to serialize request: {}", e);
            Vec::new()
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MessageError> {
        serde_json::from_slice(bytes).map_err(|e| MessageError::InvalidMessageFormat(e.to_string()))
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

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("Failed to serialize response")
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MessageError> {
        serde_json::from_slice(bytes).map_err(|e| MessageError::InvalidMessageFormat(e.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::key::Key;
    use std::net::SocketAddr;

    #[test]
    fn test_request_serialization() {
        let key = Key::new_random();
        let sender = NodeInfo::new(key, SocketAddr::new("127.0.0.2".parse().unwrap(), 8080));
        let receiver = NodeInfo::new(key, SocketAddr::new("127.0.0.1".parse().unwrap(), 8080));

        let request = Request::new(RequestType::Ping, sender.clone(), receiver.clone());
        let serialized = request.to_bytes();
        let deserialized = Request::from_bytes(&serialized).unwrap();

        assert_eq!(deserialized.request_type, RequestType::Ping);
        assert_eq!(deserialized.sender, sender);
        assert_eq!(deserialized.receiver, receiver);
    }

    #[test]
    fn test_response_serialization() {
        let key = Key::new_random();
        let sender = NodeInfo::new(key, SocketAddr::new("127.0.0.2".parse().unwrap(), 8080));
        let receiver = NodeInfo::new(key, SocketAddr::new("127.0.0.1".parse().unwrap(), 8080));
        let request_id = Uuid::new_v4();

        let response = Response::new(
            ResponseType::Pong,
            sender.clone(),
            receiver.clone(),
            request_id,
        );
        let serialized = response.to_bytes();
        let deserialized = Response::from_bytes(&serialized).unwrap();

        assert_eq!(deserialized.response_type, ResponseType::Pong);
        assert_eq!(deserialized.sender, sender);
        assert_eq!(deserialized.receiver, receiver);
        assert_eq!(deserialized.request_id, request_id);
    }
}
