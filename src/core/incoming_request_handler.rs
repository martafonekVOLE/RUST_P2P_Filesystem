use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::routing::routing_table::RoutingTable;
use crate::utils::logging::{log_error, log_warn};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{oneshot, RwLock};
use tokio::time::Instant;
use crate::storage::storage_manager::StorageManager;
use crate::config::Config;

///
/// Handles incoming requests.
///
pub async fn handle_received_request(
    request: Request,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
    storage_manager: &StorageManager,
) {
    match request.request_type {
        RequestType::Ping => {
            handle_ping_message(request, routing_table, message_dispatcher).await;
        }
        RequestType::FindNode { node_id } => {
            handle_find_node_message(request, node_id, routing_table, message_dispatcher).await;
        }
        RequestType::Store => {
            handle_store_request(request, message_dispatcher, storage_manager)
        }
        _ => {
            todo!()
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
    let closest_nodes = match routing_table.read().await.get_k_closest(&node_id) {
        Ok(nodes) => nodes,
        Err(e) => {
            log_error(&format!(
                "Failed to get K closest nodes for FIND_NODE request: {}",
                e
            ));
            return;
        }
    };

    // Respond with the closest nodes.
    let response = Response::new(
        ResponseType::new_nodes(closest_nodes).unwrap(),
        receiver.clone(),
        sender.clone(),
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        log_error(&format!("Failed to send FIND_NODE response: {}", e));
    }

    // Update routing table with the sender's info.
    record_possible_neighbour(routing_table, &sender).await;
}

///
/// Updates the routing table with the given node's information.
///
async fn record_possible_neighbour(routing_table: Arc<RwLock<RoutingTable>>, node: &NodeInfo) {
    let mut routing_table = routing_table.write().await;
    routing_table.store_nodeinfo(node.clone());
}

async fn handle_store_request(request: Request, message_dispatcher: Arc<MessageDispatcher>, storage_manager: &StorageManager){
    let (sender, receiver) = oneshot::channel::<Request>();

    // Get free port
    // todo refactor
    let free_port = TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
    let sender = request.sender.clone();
    let receiver = request.receiver;

    let response = Response::new(
        ResponseType::StoreOk { port },
        sender,
        receiver,
        request.request_id
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        log_error(&format!("Failed to send STORE OK: {}", e));
    }

    thread::spawn(async move || {
        let listener = TcpListener::bind(format!("0.0.0.0:{}", free_port)).await.expect("Unable to open TCP Listener.");

        // Fail after this duration, if no device communicates
        let timeout = Duration::from_secs(10);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                // handle timeout
            }

            match listener.accept() {
                Ok((stream, _)) => {
                    let mut buffer = Vec::new();
                    stream.read_to_end(&mut buffer).unwrap();

                    // parse data
                    let request = Request::from_bytes(buffer);

                    // handle data
                }
                Err(e) => {
                    todo!()
                }
            }

        }
    });


}

async fn handle_received_data(storage_manager: &StorageManager, request: Request) {
    // todo -> handle request data

    // Storage manager -> store
}
