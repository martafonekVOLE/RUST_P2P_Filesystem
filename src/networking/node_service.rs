use crate::networking::message_sender::MessageSender;
use crate::networking::node_info::NodeInfo;
use crate::routing::kademlia_messages::{KademliaMessage, KademliaMessageType};
use crate::routing::routing_table::RoutingTable;
use std::sync::Arc;
use tokio::sync::RwLock;

///
/// Public received messages Handler
///
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

///
/// Handles received PING messages
///
async fn handle_ping_message(
    original_message: KademliaMessage,
    routing_table: Arc<RwLock<RoutingTable>>,
) {
    let original_receiver = original_message.get_receiver();
    let original_sender = original_message.get_sender();

    let original_receiver_address = original_receiver.get_address();

    // Send PONG response if original_sender is specified
    // Also modify Node's recent activity table
    if let Some(sender) = original_sender {
        let message_sender =
            MessageSender::new(original_receiver_address.to_string().as_str()).await;
        message_sender.send_pong(sender).await;

        add_recent_activity(routing_table, sender);
    }
}

///
/// Handles received PONG messages
///
fn handle_pong_message(message: KademliaMessage, routing_table: Arc<RwLock<RoutingTable>>) {
    let original_sender = message.get_sender();

    // If sender is known, update table
    if let Some(sender) = original_sender {
        add_recent_activity(routing_table, sender);
    }
}

///
/// Handles ....
///
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

///
/// Handles ...
///
fn handle_nodes_message(message: KademliaMessage) {}

///
/// Does modify Current Node's routing table by
/// creating new or updating the state of lately used nodes
///
fn add_recent_activity(routing_table: Arc<RwLock<RoutingTable>>, contact_node: &NodeInfo) {

    // TODO:
}
