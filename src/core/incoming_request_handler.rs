use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::networking::tcp_listener::TcpListenerService;
use crate::routing::routing_table::RoutingTable;
use crate::storage::shard_storage_manager::ShardStorageManager;
use anyhow::bail;
use futures::future::err;
use futures::FutureExt;
use log::__private_api::loc;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio::time::timeout;

const LOCALHOST: &str = "127.0.0.1:0";
use super::node::Node;

///
/// Handles incoming requests.
///
pub async fn handle_received_request(
    this_node: Arc<Node>,
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
    record_possible_neighbour(this_node, routing_table.clone(), &request.sender).await;

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
        RequestType::Store { chunk_id: file_id } => {
            handle_store_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                file_id,
            )
            .await;
        }
        RequestType::GetPort { chunk_id: file_id } => {
            handle_get_port_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                file_id, // TODO: rename to chunk_hash?
            )
            .await;
        }
        RequestType::FindValue { chunk_id } => {
            handle_find_value_message(
                this_node_info,
                request,
                chunk_id,
                routing_table,
                shard_storage_manager,
                message_dispatcher,
            )
            .await;
        }
        RequestType::GetValue { chunk_id, port } => {
            handle_get_value(request, port).await;
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
///
///
async fn handle_find_value_message(
    this_node_info: NodeInfo,
    request: Request,
    target: Key,
    routing_table: Arc<RwLock<RoutingTable>>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    message_dispatcher: Arc<MessageDispatcher>,
) {
    let chunk_exists = shard_storage_manager
        .read()
        .await
        .is_chunk_already_stored(&target);

    if !chunk_exists {
        handle_find_node_message(
            this_node_info,
            request,
            target,
            routing_table,
            message_dispatcher,
        )
        .await;
        return;
    }

    // Respond with a PONG message.
    let response = Response::new(
        ResponseType::HasValue { chunk_id: target },
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver fiel
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send HAS_VALUE response: {}", e);
    }
}

///
///
///
async fn handle_get_value(request: Request, port: u16) {
    // Get chunk from storage and convert it to data (Vec<u8>)
    let data = Vec::new(); // TODO: get chunk from storage

    // This will get the chunk from storage
    let address = format!("{}:{}", request.sender.address.ip(), port);
    let mut stream = match TcpStream::connect(&address).await {
        Ok(s) => s,
        Err(e) => {
            error!("Unable to connect to the TCP Stream at {}: {}", address, e);
            return;
        }
    };

    if let Err(e) = stream.write_all(data.as_slice()).await {
        error!("Unable to write data to TCP Stream at {}: {}", address, e);
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

    let response = if !chunk_exists {
        Response::new(
            ResponseType::StoreOK,
            this_node_info,         // Sender field.
            request.sender.clone(), // Receiver field.
            request.request_id,
        )
    } else {
        if let Err(e) = shard_storage_manager
            .write()
            .await
            .update_chunk_upload_time(&file_id)
        {
            error!("Unable to update TTL for chunk {}: {}", file_id, e);
        }
        Response::new(
            ResponseType::StoreChunkUpdated,
            this_node_info,         // Sender field.
            request.sender.clone(), // Receiver field.
            request.request_id,
        )
    };

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send STORE response: {}", e);
    }
}

async fn handle_get_port_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    file_id: Key,
) {
    let tcp_listener_service = match TcpListenerService::new().await {
        Ok(listener) => listener,
        Err(e) => {
            error!("TCP Listener failed: {}", e);
            return;
        }
    };

    let port = tcp_listener_service.get_port();

    if let Some(insert_error) = shard_storage_manager
        .write()
        .await
        .add_active_tcp_connection(port, request.sender.clone(), file_id)
    {
        error!(
            "Unable to store a connection for file_id {} on port {}: {:?}",
            file_id, port, insert_error
        );
        return;
    }

    let response = Response::new(
        ResponseType::PortOK { port },
        this_node_info,         // Sender field.
        request.sender.clone(), // Receiver field.
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send PORT_OK response: {}", e);
        return;
    }

    tokio::spawn(async move {
        match tcp_listener_service.receive_data().await {
            Ok(data) => handle_tcp_upload(data, port, &shard_storage_manager).await,
            Err(e) => error!(
                "Unable to receive data from TCP listener on port {}: {}",
                port, e
            ),
        }
    });
}

///
/// Stores the received data to the storage.
///
async fn handle_tcp_upload(
    data: Vec<u8>,
    port: u16,
    shard_storage_manager: &Arc<RwLock<ShardStorageManager>>,
) {
    if let Err(e) = shard_storage_manager
        .write()
        .await
        .store_chunk_for_known_peer(data, port)
        .await
    {
        error!("Failed to save shard to storage on port {}: {}", port, e);
        return;
    }

    info!("Successfully stored the received data on port {}.", port);
}

///
/// Updates the routing table with the given node's information.
///
async fn record_possible_neighbour(
    this_node: Arc<Node>,
    routing_table: Arc<RwLock<RoutingTable>>,
    node: &NodeInfo,
) {
    let mut routing_table = routing_table.write().await;
    // routing_table.store_nodeinfo(node.clone()).unwrap();
    match routing_table
        .store_nodeinfo(node.clone(), this_node.as_ref())
        .await
    {
        Ok(_) => {}
        Err(e) => {
            error!("Failed to store node info: {}", e);
        }
    }
}
