use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::routing::routing_table::RoutingTable;
use log::{error, info, warn};
use std::sync::Arc;
use tokio::sync::RwLock;

///
/// Handles incoming requests.
///
pub async fn handle_received_request(
    this_node_info: NodeInfo,
    request: Request,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    info!("Received request: {}", request);

    if request.receiver.id != this_node_info.id {
        warn!(
            "Received a request for wrong node: {} (This is node {})",
            request, this_node_info
        );
        return;
    }

    // Update routing table with the sender's info.
    record_possible_neighbour(routing_table.clone(), &request.sender).await;

    match request.request_type {
        RequestType::Ping => {
            handle_ping_message(this_node_info, request, message_dispatcher).await;
        }
        RequestType::FindNode { node_id } => {
            handle_find_node_message(
                this_node_info,
                request,
                node_id,
                routing_table,
                message_dispatcher,
            )
            .await;
        }
    }
}

///
/// Handles PING requests.
/// Sends a PONG response and updates the routing table with the sender.
///
async fn handle_ping_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    // Respond with a PONG message.
    let response = Response::new(
        ResponseType::Pong,
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver field
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send PONG response: {}", e);
    }
}

///
/// Handles FIND_NODE requests.
/// Returns the K closest nodes to the requested `node_id`.
///
async fn handle_find_node_message(
    this_node_info: NodeInfo,
    request: Request,
    target: Key,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    // Fetch the K closest nodes to the `node_id` from the routing table.
    let closest_nodes = match routing_table.read().await.get_k_closest(&target) {
        Ok(nodes) => nodes,
        Err(e) => {
            error!("Failed to get K closest nodes for FIND_NODE request: {}", e);
            return;
        }
    };

    // Respond with the closest nodes.
    let response = Response::new(
        ResponseType::new_nodes(closest_nodes).unwrap(),
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver field
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send FIND_NODE response: {}", e);
    }
}

///
/// Updates the routing table with the given node's information.
///
async fn record_possible_neighbour(routing_table: Arc<RwLock<RoutingTable>>, node: &NodeInfo) {
    let mut routing_table = routing_table.write().await;
    routing_table.store_nodeinfo(node.clone()).unwrap();
}
