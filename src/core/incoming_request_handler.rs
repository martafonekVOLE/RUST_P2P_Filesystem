use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::routing::routing_table::RoutingTable;
use crate::utils::logging::log_warn;
use std::sync::Arc;
use tokio::sync::RwLock;

///
/// Handles incoming requests.
///
pub async fn handle_received_request(
    request: Request,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    match request.request_type {
        RequestType::Ping => {
            handle_ping_message(request, routing_table, message_dispatcher).await;
        }
        RequestType::FindNode { node_id } => {
            handle_find_node_message(request, node_id, routing_table, message_dispatcher).await;
        }
    }
}

///
/// Handles PING requests.
/// Sends a PONG response and updates the routing table with the sender.
///
async fn handle_ping_message(
    request: Request,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    let sender = request.sender.clone();
    let receiver = request.receiver;

    // Respond with a PONG message.
    let response = Response::new(
        ResponseType::Pong,
        receiver.clone(),
        sender.clone(),
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        eprintln!("Failed to send PONG response: {}", e);
    }

    // Update routing table with the sender's info.
    record_possible_neighbour(routing_table, &sender).await;
}

///
/// Handles FIND_NODE requests.
/// Returns the K closest nodes to the requested `node_id`.
///
async fn handle_find_node_message(
    request: Request,
    node_id: crate::core::key::Key,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    if request.receiver.id != node_id {
        log_warn(&format!(
            "Received FIND_NODE request for wrong node ID: {}",
            node_id
        ));
        return;
    }

    let sender = request.sender.clone();
    let receiver = request.receiver;

    // Fetch the K closest nodes to the `node_id` from the routing table.
    {
        let routing_table = routing_table.read().await;
        // TODO implement
        // routing_table.get_closest_nodes(&node_id)
    };

    // Respond with the closest nodes.
    let response = Response::new(
        ResponseType::Pong, // Replace with ResponseType::Nodes
        receiver.clone(),
        sender.clone(),
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        eprintln!("Failed to send NODES response: {}", e);
    }

    // Update routing table with the sender's info.
    record_possible_neighbour(routing_table, &sender).await;
}

///
/// Updates the routing table with the given node's information.
/// TODO This method should belong to the RoutingTable struct.
///
async fn record_possible_neighbour(routing_table: Arc<RwLock<RoutingTable>>, node: &NodeInfo) {
    let routing_table = routing_table.write().await;
    // TODO routing_table.update_with_node(node.clone());
}
