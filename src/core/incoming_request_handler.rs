use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::routing::routing_table::RoutingTable;
use crate::storage::shard_storage_manager::ShardStorageManager;
use futures::FutureExt;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::timeout;

///
/// Handles incoming requests.
///
pub async fn handle_received_request(
    this_node_info: NodeInfo,
    request: Request,
    routing_table: Arc<RwLock<RoutingTable>>,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
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
        RequestType::Store => {}
        RequestType::StorePort { file_id } => {
            handle_store_port_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                file_id,
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

async fn handle_store_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: MessageDispatcher,
) {
    // Respond with a StoreOK message.
    let response = Response::new(
        ResponseType::StoreOK,
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver field
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send StoreOK response: {}", e);
    }
}

async fn handle_store_port_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    file_id: Key,
) {
    let tcp_listener = match TcpListener::bind("127.0.0.1:0").await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Error opening TCP Listener.");
            return;
        }
    };

    let socket_address = match tcp_listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Error reading Socket Address.");
            return;
        }
    };

    let port = socket_address.port();

    // Store into active TCP connections
    if let None = shard_storage_manager
        .write()
        .await
        .add_active_tcp_connection(port, request.sender.clone(), file_id.clone())
    {
        error!("Unable to receive data, nothing was saved to active TCP connections table.")
    }

    // Respond with a STORE_OK message.
    let response = Response::new(
        ResponseType::StorePortOK { port },
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver field
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send StorePortOk response: {}", e);
        return;
    }

    // Wait for TCP stream
    tokio::spawn(async move {
        match timeout(Duration::from_secs(10), tcp_listener.accept()).await {
            Ok(Ok((mut stream, addr))) => {
                let mut data = Vec::new();
                let received_bytes = stream.read_to_end(&mut data).await;

                match received_bytes {
                    Ok(_) => {
                        shard_storage_manager
                            .write()
                            .await
                            .save_for_port(data, port)
                            .expect("Failed to save shard to storage");
                    }
                    Err(_) => {
                        error!("An error has occurred while reading the TCP Stream.");
                        return;
                    }
                }
            }
            Ok(Err(err)) => {
                error!("Failure while accepting data.");
                return;
            }
            Err(_) => {
                error!("Timed out while waiting for connection.");
                return;
            }
        }
    });
}

///
/// Updates the routing table with the given node's information.
///
async fn record_possible_neighbour(routing_table: Arc<RwLock<RoutingTable>>, node: &NodeInfo) {
    let mut routing_table = routing_table.write().await;
    routing_table.store_nodeinfo(node.clone());
}
