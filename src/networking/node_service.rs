use crate::core::node;
use crate::core::node::Node;
use crate::networking::message_sender;
use crate::networking::message_sender::MessageSender;
use crate::networking::node_info::NodeInfo;
use crate::routing::kademlia_messages::{KademliaMessage, KademliaMessageType};
use crate::routing::routing_table::RoutingTable;
use std::process::exit;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn handle_received_request(
    message: KademliaMessage,
    routing_table: Arc<RwLock<RoutingTable>>,
) {
    let kademlia_message_type = message.get_type();

    match *kademlia_message_type {
        KademliaMessageType::Ping => {
            handle_ping_message(message, routing_table).await;
        }
        KademliaMessageType::FindNode { .. } => {
            handle_find_node_message(message, routing_table);
        }
        KademliaMessageType::Pong => {
            handle_pong_message(message, routing_table);
        }
        KademliaMessageType::Nodes { .. } => {
            handle_nodes_message(message);
        }
    }
}

async fn handle_ping_message(original_message: KademliaMessage, routing_table: Arc<RwLock<RoutingTable>>) {
    let original_receiver = original_message.get_receiver();
    let original_sender = original_message.get_sender();

    let original_receiver_address = original_receiver.get_address_unwrapped();

    // Modify Node's recent activity -> add node to KBuckets
    add_recent_activity(routing_table, original_sender);

    // Send PONG response
    let message_sender = MessageSender::new(original_receiver_address.to_string().as_str()).await;
    message_sender.send_pong(original_sender).await;
}

fn handle_pong_message(message: KademliaMessage, routing_table: Arc<RwLock<RoutingTable>>) {
    let sender = message.get_sender();
    add_recent_activity(routing_table, sender);
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

fn add_recent_activity(routing_table: Arc<RwLock<RoutingTable>>, contact_node: &NodeInfo) {

    // TODO:
}
