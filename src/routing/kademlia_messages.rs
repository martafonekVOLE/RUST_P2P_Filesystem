use crate::config::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::process::exit;

#[derive(Serialize, Deserialize)]
pub enum KademliaMessageType {
    Ping,
    Pong,
    // For looking up a node
    FindNode { node_id: Key },
    // Response for FindNOde message - returns K closest nodes to the queried node_id
    Nodes { nodes: [Option<NodeInfo>; K] },
}

impl KademliaMessageType {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            KademliaMessageType::Ping => vec![0x01],
            KademliaMessageType::Pong => vec![0x02],
            KademliaMessageType::FindNode { node_id } => {
                let mut bytes = vec![0x03];
                bytes.extend(node_id.to_bytes());
                bytes
            }
            KademliaMessageType::Nodes { nodes } => {
                let mut bytes = vec![0x04];
                for node in nodes.iter() {
                    match node {
                        Some(node_info) => bytes.extend(node_info.to_bytes()),
                        None => bytes.push(0x00),
                    }
                }
                bytes
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct KademliaMessage {
    msg_type: KademliaMessageType,
    sender: NodeInfo,
    receiver: NodeInfo,
    magic_cookie: String,
}

impl KademliaMessage {
    pub fn new(msg_type: KademliaMessageType, sender: NodeInfo, receiver: NodeInfo) -> Self {
        KademliaMessage {
            msg_type,
            sender,
            receiver,
            magic_cookie: String::new(),
        }
    }

    pub fn get_type(&self) -> &KademliaMessageType {
        &self.msg_type
    }
}

pub fn parse_kademlia_message(message: Cow<str>) -> KademliaMessage {
    let msg = serde_json::from_str(&message);
    let parsed_message: KademliaMessage = match msg {
        Ok(msg) => msg,
        Err(e) => {
            println!("Error while receiving Kademlia message: {}", e);
            exit(1);
        }
    };

    parsed_message
}
