use super::node::Node;
use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::networking::tcp_listener::TcpListenerService;
use crate::routing::routing_table::RoutingTable;
use crate::storage::shard_storage_manager::ShardStorageManager;
use log::{error, info, warn};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::RwLock;

///
/// # Router for handling incoming requests.
///
/// This method is the entry point for all incoming requests and is called from the main receive loop
/// of the node. A new task for this method call should be spawned for each incoming request to
/// handle it asynchronously.
///
/// This method also updates the routing table with the sender's information, which ensures that the
/// node has the most up-to-date information about the nodes it communicates with.
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
        RequestType::Store { chunk_id } => {
            handle_store_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                chunk_id,
            )
            .await;
        }
        RequestType::GetPort { chunk_id } => {
            handle_get_port_message(
                this_node_info,
                request,
                message_dispatcher,
                shard_storage_manager,
                chunk_id,
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
            handle_get_value(request, chunk_id, port, shard_storage_manager).await;
        }
    }
}

///
/// Handles `PING` requests by responding with a `PONG` message.
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
/// Handles `FIND_NODE` requests.
///
/// Returns the `K` closest nodes to the requested `node_id`.
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
/// Handles `FIND_VALUE` requests.
/// Returns the K closest nodes to the requested `key` when
/// none of the nodes has direct access to a file with the
/// same key or a single node which has the key.
///
/// For single node, this returns `HAS_VALUE`.
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

    // If the chunk is not present in the storage, it behaves
    // like response for FIND_NODE.
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

    // Respond with a HAS_VALUE message.
    let response = Response::new(
        ResponseType::HasValue { chunk_id: target },
        this_node_info,         // Sender field
        request.sender.clone(), // Receiver field
        request.request_id,
    );

    if let Err(e) = message_dispatcher.send_response(response).await {
        error!("Failed to send HAS_VALUE response: {}", e);
    }
}

///
/// Handles `GET_VALUE` requests.
/// Receiving that request type does instruct the node,
/// that it will transfer some of its data to the
/// requesting node.
///
/// Receiving node connects to a TCP stream and starts
/// sending requested data.
///
///
async fn handle_get_value(
    request: Request,
    chunk_id: Key,
    port: u16,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
) {
    // Get chunk from storage and convert it to bytes
    let data = match shard_storage_manager
        .read()
        .await
        .read_chunk(&chunk_id)
        .await
    {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to get chunk from storage: {}", e);
            return;
        }
    };

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
/// Handles `STORE` request.
///
/// Saves received chunk to the file system and adds entry to `ShardStorageManager`'s map
///
async fn handle_store_message(
    this_node_info: NodeInfo,
    request: Request,
    message_dispatcher: Arc<MessageDispatcher>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
    file_id: Key,
) {
    // Check first if chunk with this hash is already stored,
    // in this case send response that chunk is already stored and just update TTL.
    let chunk_exists = shard_storage_manager
        .read()
        .await
        .is_chunk_already_stored(&file_id);

    // If we don't have this chunk, store it.
    let response = if !chunk_exists {
        Response::new(
            ResponseType::StoreOK,
            this_node_info,         // Sender field.
            request.sender.clone(), // Receiver field.
            request.request_id,
        )
    // If we already have the chunk, update its TTL.
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
        error!("Failed to send StoreOK response: {}", e);
    }
}

///
/// Handles `GET_PORT` message.
///
/// This request type does instruct node to open a TCP Listener
/// and provide the other node with port. The other node should
/// send data over this TCP connection.
///
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

    // Respond with a PORT_OK message.
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

    // Wait for TCP stream
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
/// This method handles chunks, which were received over the established TCP connection.
/// Chunks are stored using the `ShardStorageManager`.
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
