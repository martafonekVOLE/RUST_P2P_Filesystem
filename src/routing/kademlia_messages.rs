use serde::{Deserialize, Serialize};
use std::borrow::Cow;

#[derive(Serialize, Deserialize)]
pub enum KademliaMessageType {
    Ping,
    Store,
    FindValue,
    FindNode,
}

#[derive(Serialize, Deserialize)]
pub struct KademliaMessage {
    msg_type: KademliaMessageType,
    sender: String,
    receiver: String,
    magic_cookie: String,
}

pub fn build_kademlia_message(
    msg_type: KademliaMessageType,
    sender_id: String,
    receiver_id: String,
) -> String {
    // todo add magic cookie
    let message = KademliaMessage {
        msg_type,
        sender: sender_id,
        receiver: receiver_id,
        magic_cookie: String::new(),
    };

    let json = serde_json::to_string(&message);

    match json {
        Ok(jso) => jso.to_string(),
        Err(e) => {
            println!("Error while creating Kademlia message: {}", e);
            String::new()
        }
    }
}

pub fn parse_kademlia_message(message: Cow<str>) -> Option<KademliaMessage> {
    let msg = serde_json::from_str(&message);
    let parsed_message: Option<KademliaMessage> = match msg {
        Ok(msg) => Some(msg),
        Err(e) => {
            println!("Error while receiving Kademlia message: {}", e);
            return None;
        }
    };

    parsed_message
}
