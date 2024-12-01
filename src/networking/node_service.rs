use crate::networking::message_sender::MessageSender;
use crate::routing::kademlia_messages::{KademliaMessage, KademliaMessageType};
use crate::routing::routing_table::RoutingTable;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn handle_received_request(
    message: KademliaMessage,
    routing_table: Arc<RwLock<RoutingTable>>,
) {
    let kademlia_message_type = message.get_type();

    match *kademlia_message_type {
        KademliaMessageType::Ping => {
            handle_ping_message(message);
        }
        KademliaMessageType::FindNode { .. } => {
            handle_find_node_message(message, routing_table);
        }
        KademliaMessageType::Pong => {
            handle_pong_message(message);
        }
        KademliaMessageType::Nodes { .. } => {
            handle_nodes_message(message);
        }
    }
}

fn handle_ping_message(message: KademliaMessage) {
    // send pong
}

fn handle_pong_message(message: KademliaMessage) {
    // send pong
}

fn handle_find_node_message(message: KademliaMessage, routing_table: Arc<RwLock<RoutingTable>>) {
    // get from table
    // respond
    // call something, that does:
    // 1. get nodes from table
    // 2. returns K closest nodes
    // 3. send back K closest nodes

    //message_sender::respond_with_nodes(nodes);
    //
}

fn handle_nodes_message(message: KademliaMessage) {}

pub async fn send_message(message: KademliaMessage) {
    // handle send message
}
