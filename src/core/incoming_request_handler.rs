use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::routing::routing_table::RoutingTable;
use crate::storage::shard_storage_manager::ShardStorageManager;
use futures::future::err;
use futures::FutureExt;
use log::__private_api::loc;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::timeout;

const LOCALHOST: &str = "127.0.0.1:0";

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
        RequestType::Store { file_id } => {
            handle_store_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                file_id,
            )
            .await;
        }
        RequestType::GetPort { file_id, is_store } => {
            handle_get_port_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                file_id, // TODO: rename to chunk_hash?
                is_store,
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
/// Handles STORE request
/// Saves received chunk to the file system and adds entry to ShardManager's map
///
async fn handle_store_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    file_id: Key,
) {
    // TODO: check first if chunk with this hash is already stored,
    // in this case send response that chunk is already stored and just update TTL or smth.
    let chunk_exists = shard_storage_manager
        .read()
        .await
        .is_chunk_already_stored(&file_id);
    let response = match chunk_exists {
        false => {
            Response::new(
                ResponseType::StoreOK,
                this_node_info,         // Sender field
                request.sender.clone(), // Receiver field
                request.request_id,
            )
        }
        true => {
            shard_storage_manager
                .write()
                .await
                .update_chunk_upload_time(&file_id)
                .expect("Unable to update TTL.");

            Response::new(
                ResponseType::StoreChunkUpdated,
                this_node_info,         // Sender field
                request.sender.clone(), // Receiver field
                request.request_id,
            )
        }
    };

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send StoreOK response: {}", e);
    }
}

async fn handle_get_port_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    file_id: Key,
    is_upload: bool,
) {
    // Open TCP Listener for data transfer
    let tcp_listener = match TcpListener::bind(LOCALHOST).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Error opening TCP Listener.");
            return;
        }
    };

    // Get available port
    let port = match tcp_listener.local_addr() {
        Ok(addr) => addr,
        Err(e) => {
            error!("Error reading Socket Address.");
            return;
        }
    }
    .port();

    // Store node into active TCP connections
    if let Some(insert) = shard_storage_manager
        .write()
        .await
        .add_active_tcp_connection(port, request.sender.clone(), file_id.clone())
    {
        error!("Unable to store a connection.");
        return;
    }

    // Respond with a PORT_OK message.
    let response = Response::new(
        ResponseType::PortOK { port },
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
            Ok(Ok((mut stream, _addr))) => {
                info!("Established TCP connection.");

                let mut data = Vec::new();
                let received_bytes = stream.read_to_end(&mut data).await;

                match received_bytes {
                    Ok(_) => {
                        info!("Successfully received the data over TCP.");

                        match is_upload {
                            true => handle_tcp_upload(data, port, &shard_storage_manager).await,
                            false => handle_tcp_download().await,
                        };
                    }
                    Err(_) => {
                        error!("An error has occurred while reading the TCP Stream.");
                        return;
                    }
                }
            }
            Ok(Err(_err)) => {
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

async fn handle_tcp_download() {
    todo!()
}

///
///
///
async fn handle_tcp_upload(
    data: Vec<u8>,
    port: u16,
    shard_storage_manager: &Arc<RwLock<ShardStorageManager>>,
) {
    shard_storage_manager
        .write()
        .await
        .store_chunk_for_known_peer(data, port)
        .await
        .expect("Failed to save shard to storage");

    info!("Successfully stored the received data.");
}

///
/// Updates the routing table with the given node's information.
///
async fn record_possible_neighbour(routing_table: Arc<RwLock<RoutingTable>>, node: &NodeInfo) {
    let mut routing_table = routing_table.write().await;
    // routing_table.store_nodeinfo(node.clone()).unwrap();
    match routing_table.store_nodeinfo(node.clone()) {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to store node info: {}", e);
        }
    }
}
