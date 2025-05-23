use crate::constants::{ALPHA, LOOKUP_TIMEOUT_MILLISECONDS};
use crate::constants::{BASIC_REQUEST_TIMEOUT_MILLISECONDS, RECEIVE_WORKER_TASK_COUNT};
use crate::core::key::Key;
use crate::core::lookup::{LookupBuffer, LookupResponse, LookupSuccessType};
use crate::core::request_handler::handle_received_request;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType, MAX_MESSAGE_SIZE};
use crate::networking::node_info::NodeInfo;
use crate::networking::request_map::RequestMap;
use crate::routing::routing_table::{RoutingTable, RoutingTableError};
use crate::sharding::common::{Chunk, FileMetadata};
use crate::sharding::uploader::FileUploader;
use crate::storage::file_manager::FileManager;
use crate::storage::shard_storage_manager::ShardStorageManager;
use futures::future::join_all;
use log::{error, info, warn};
use std::cmp::PartialEq;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, UdpSocket as TokioUdpSocket};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task;

use anyhow::{bail, Context, Result};

use crate::networking::tcp_listener::TcpListenerService;
use crate::sharding::downloader::FileDownloader;
use thiserror::Error;
use tokio::time::timeout;

#[derive(Error, Debug, PartialEq)]
pub enum NodeError {
    #[error("No response received on the channel")]
    BadResponse,
    #[error("Timed out awaiting response")]
    ResponseTimeout,
    #[error("Unable to send request via dispatcher")]
    UnableToSend,
    #[error("PING failed. Node not in routing table")]
    PingMissingNode,
    #[error("Routing table failed")]
    RoutingTableFailed(#[from] RoutingTableError),
}

pub struct Node {
    /// Main identifier of the node
    pub key: Key,
    /// Address and port of the node
    pub address: SocketAddr,
    /// UDP socket of the node used for incoming traffic - bound on the `node.address`
    socket: Arc<TokioUdpSocket>,
    /// Routing table of the node
    pub routing_table: Arc<RwLock<RoutingTable>>,
    // Map of pending requests sent by this node. Thread safe by design ( Arc<RwLock<HashMap>> inside)
    request_map: RequestMap,
    // Provides operations to send messages to other nodes
    message_dispatcher: Arc<MessageDispatcher>,
    // Manages user's file uploads
    file_manager: Arc<FileManager>,
    // Manages chunks stored by other nodes in our storage
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
}

impl Node {
    ///
    /// Default constructor for the Node struct.
    ///
    pub async fn new(key: Key, ip: String, port: u16, storage_path: String) -> Result<Self> {
        let address: SocketAddr = format!("{}:{}", ip, port)
            .parse()
            .with_context(|| format!("Parsing socket address from {}:{}", ip, port))?;
        let socket = TokioUdpSocket::bind(address)
            .await
            .with_context(|| format!("Binding UDP socket to address {}", address))?;
        let resolved_address = socket
            .local_addr()
            .with_context(|| "Retrieving the local socket address after binding")?;
        let message_dispatcher = Arc::new(
            MessageDispatcher::new()
                .await
                .with_context(|| "Failed to create MessageDispatcher")?,
        );

        Ok(Node {
            key,
            address: resolved_address,
            socket: Arc::new(socket),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(key))),
            request_map: RequestMap::new(),
            message_dispatcher,
            file_manager: Arc::new(FileManager::new()),
            shard_storage_manager: Arc::new(RwLock::new(ShardStorageManager::new(PathBuf::from(
                storage_path,
            )))),
        })
    }

    ///
    /// This method converts Node to NodeInfo.
    ///
    pub fn to_node_info(&self) -> NodeInfo {
        NodeInfo::new(self.key, self.address)
    }

    ///
    /// Dumps the routing table into a string form.
    ///
    pub async fn get_routing_table_content(&self) -> Vec<NodeInfo> {
        self.routing_table.read().await.get_all_nodeinfos()
    }

    ///
    /// Returns a vector of all chunks this node stores.
    ///
    pub async fn get_owned_chunk_keys(&self) -> Vec<Key> {
        self.shard_storage_manager
            .read()
            .await
            .get_owned_chunk_keys()
    }

    // ------------------------------- MAIN PUBLIC METHODS ---------------------------------- //

    ///
    /// Creates a listening task that indefinitely waits for incoming messages on this node's socket.
    /// Incoming messages are enqueued into a bounded channel and processed by a fixed pool of workers.
    ///
    /// This must be called after the node is initialized, otherwise the node won't be able to receive
    /// any messages.
    ///
    /// This method is non-blocking and runs in the background.
    ///
    pub fn start_listening(self: Arc<Self>) {
        // Clone the necessary shared resources.
        let socket = Arc::clone(&self.socket);
        let routing_table = Arc::clone(&self.routing_table);
        let request_map = self.request_map.clone();
        let message_dispatcher = Arc::clone(&self.message_dispatcher);
        let shard_storage_manager = Arc::clone(&self.shard_storage_manager);
        let this_node_info = self.to_node_info();
        let node_clone = self.clone();

        // Define the capacity of the incoming message queue.
        const QUEUE_CAPACITY: usize = 100;

        // Create a bounded MPSC channel for incoming messages.
        // Each message is a tuple: (data: Vec<u8>, source: SocketAddr).
        let (tx, rx) = mpsc::channel::<(Vec<u8>, SocketAddr)>(QUEUE_CAPACITY);

        // Wrap the receiver in an Arc<Mutex<>> so that multiple worker tasks can share it.
        let rx = Arc::new(Mutex::new(rx));

        // Spawn the task that continuously reads from the UDP socket
        // and enqueues incoming messages.
        task::spawn({
            let tx = tx.clone();
            async move {
                let mut buffer = [0u8; MAX_MESSAGE_SIZE];
                loop {
                    match socket.recv_from(&mut buffer).await {
                        Ok((size, src)) => {
                            let data = buffer[..size].to_vec();
                            if let Err(e) = tx.send((data, src)).await {
                                error!("Failed to enqueue incoming message: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("Error receiving message: {}", e);
                        }
                    }
                }
            }
        });

        // Spawn worker tasks to process messages from the queue.
        for _ in 0..RECEIVE_WORKER_TASK_COUNT {
            let rx = Arc::clone(&rx);
            let routing_table = Arc::clone(&routing_table);
            let request_map = request_map.clone();
            let message_dispatcher = Arc::clone(&message_dispatcher);
            let shard_storage_manager = Arc::clone(&shard_storage_manager);
            let this_node_info = this_node_info.clone();
            let node_clone = node_clone.clone();

            task::spawn(async move {
                loop {
                    // Lock the receiver to get the next message.
                    let msg_opt = {
                        let mut rx_guard = rx.lock().await;
                        rx_guard.recv().await
                    };

                    // If the channel is closed, exit the worker.
                    let (data, src) = match msg_opt {
                        Some(msg) => msg,
                        None => break,
                    };

                    // Process the message.
                    // Try to deserialize as a Response.
                    if let Ok(response) = serde_json::from_slice::<Response>(&data) {
                        request_map.handle_response(response).await;
                    }
                    // Try to deserialize as a Request.
                    else if let Ok(request) = serde_json::from_slice::<Request>(&data) {
                        handle_received_request(
                            node_clone.clone(),
                            this_node_info.clone(),
                            request,
                            Arc::clone(&routing_table),
                            Arc::clone(&message_dispatcher),
                            Arc::clone(&shard_storage_manager),
                        )
                        .await;
                    }
                    // Log an error if the message cannot be parsed.
                    else {
                        error!("Failed to parse message from {}", src);
                    }
                }
            });
        }
    }

    ///
    /// Sends a PING request to the specified node ID.
    /// Awaits a PONG response and returns whether the node is reachable.
    ///
    pub async fn ping(&self, node_id: Key) -> Result<Response, NodeError> {
        // Get the address from the RoutingTable
        let receiver_info = match self.routing_table.read().await.get_nodeinfo(&node_id)? {
            Some(info) => info.clone(),
            None => {
                return Err(NodeError::PingMissingNode);
            }
        };

        let request = Request::new(RequestType::Ping, self.to_node_info(), receiver_info);

        // Just call the send_request_and_wait method, it will return err if the request fails
        self.send_request_and_wait(request, BASIC_REQUEST_TIMEOUT_MILLISECONDS)
            .await
    }

    ///
    /// Performs the join network procedure to join an existing network.
    ///
    /// After successfully joining the network, the responsive nodes are stored in the routing table
    /// and the Node is ready to participate in the network.
    ///
    /// Returns a vector of K closest nodes it has found during the lookup procedure.
    ///
    pub async fn join_network(&self, beacon_node: NodeInfo) -> Result<Vec<NodeInfo>> {
        info!(
            "Beginning join_network procedure via beacon {}",
            beacon_node.id
        );
        self.routing_table
            .write()
            .await
            .store_nodeinfo(beacon_node.clone(), self)
            .await?;

        self.ping(beacon_node.id)
            .await
            .context("Beacon node unavailable.")?;

        // Send single find_node to the beacon to get the initial k-closest nodes
        let mut initial_k_closest = self
            .single_lookup(self.key, beacon_node.clone(), false)
            .await
            .get_k_closest();

        if initial_k_closest.is_empty() {
            bail!("Failed to join network. Beacon node did not respond.");
        }

        // In case we are the second node in the network, the response only contains us. Therefore,
        // we should remove ourselves from the initial_k_closest list to prevent storing ourselves
        // in the routing table (As we would be sending message to ourselves)
        // The lookup will not be performed if we are the second node in the network.
        initial_k_closest.retain(|node| *node != self.to_node_info());

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(self.key, initial_k_closest, self.to_node_info());

        // Execute all lookup rounds
        loop {
            // Get the resolvers for the next round
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
            // This will happen if we are the second node in the network or if we exhausted all
            // the unqueried nodes.
            if resolvers.is_empty() {
                break;
            }

            // Get the responses from all resolvers in this round
            let current_responses = self.lookup_round(self.key, resolvers, false).await;

            // Record responses. If we didn't get any closer results, quit the rounds loop
            if !lookup_buffer.record_lookup_round_responses(current_responses, false) {
                break;
            }
        }

        // Execute the last lookup on all the resulting nodes that haven't been queried yet
        let last_lookup_responses = self
            .lookup_round(
                self.key,
                lookup_buffer.get_unqueried_resulting_nodes(),
                false,
            )
            .await;
        lookup_buffer.record_lookup_round_responses(last_lookup_responses, false);

        // Update routing table with the responsive nodes
        let mut routing_table = self.routing_table.write().await;
        for node_info in lookup_buffer.get_responsive_nodes() {
            routing_table.store_nodeinfo(node_info, self).await?;
        }

        info!("Successfully joined the network!");
        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// Attempts to find the closest `K` nodes in the entire network with respect to the `target` key.
    /// This method can only be used if the node is already part of the network. (the node already
    /// successfully underwent the `join_network()` procedure)
    ///
    pub async fn find_node(&self, target: Key) -> Result<Vec<NodeInfo>> {
        // Initial resolvers are the alpha closest nodes to the target in our RT
        let initial_resolvers = self
            .routing_table
            .write()
            .await
            .lookup_get_alpha_closest(&target)?;
        if initial_resolvers.is_empty() {
            bail!("No initial resolvers available for find_node resolutions");
        }

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(target, initial_resolvers, self.to_node_info());

        loop {
            // Get the resolvers for this
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
            if resolvers.is_empty() {
                break;
            }

            // Get the responses from all resolvers in this round
            let current_responses = self.lookup_round(target, resolvers, false).await;

            // Record responses. If we didn't get any closer results, quit the rounds loop
            if !lookup_buffer.record_lookup_round_responses(current_responses, false) {
                break;
            }
        }

        // Execute the last lookup on all the resulting nodes that haven't been queried yet
        let last_lookup_responses = self
            .lookup_round(target, lookup_buffer.get_unqueried_resulting_nodes(), false)
            .await;
        lookup_buffer.record_lookup_round_responses(last_lookup_responses, false);

        // Update routing table with the responsive nodes
        let mut routing_table = self.routing_table.write().await;
        for node_info in lookup_buffer.get_responsive_nodes() {
            if let Err(err) = routing_table
                .store_nodeinfo(node_info.clone(), self)
                .await
                .with_context(|| {
                    format!("Failed to store nodeinfo {} in routing table", node_info.id)
                })
            {
                warn!(
                    "Could not insert potential new peer to the routing table: {:?}",
                    err
                );
            }
        }

        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// This method handles file uploads in multiple steps.
    /// First, it checks whether the requested file exists. If it does, the file is split
    /// into small chunks, which are then stored in the network one by one.
    ///
    /// For each chunk, the `FIND_NODE` method is called, and from the responding nodes,
    /// the `ALPHA` closest ones (based on the XOR metric) are selected. A `GET_PORT`
    /// message is then sent to these `ALPHA` nodes, instructing them to provide a port for
    /// a TCP connection. The data is subsequently transferred to all nodes that provided
    /// a port.
    ///
    pub async fn upload_file(&self, file_path: &str) -> Result<FileMetadata> {
        // Check file existence
        if !Path::new(file_path).exists() {
            bail!(
                "Can't upload nonexistent file '{}'. Did you enter the correct path?",
                file_path
            );
        }

        // Start sharding
        let mut uploader = FileUploader::new(file_path).await?;

        // Loop over shards
        loop {
            // Get next shard
            let chunk = uploader.get_next_chunk_encrypt().await?;

            match chunk {
                Some(chunk) => {
                    // Get the ALPHA closest nodes responsible for the chunk
                    let responsible_nodes = self
                        .resolve_alpha_closest_nodes_to_key(chunk.clone().hash)
                        .await?;

                    // Get ports for TCP data transfer
                    let ports = self
                        .get_ports_for_nodes(responsible_nodes, chunk.clone().hash)
                        .await?;

                    // Send data to each node over TCP
                    self.store_chunk(ports.clone(), chunk.clone()).await?;
                }
                None => {
                    break;
                }
            }
        }

        // Save info to FileManager
        self.file_manager.add_file_uploaded_entry(file_path).await;

        // Get the file metadata and return it
        uploader
            .get_metadata()
            .await
            .with_context(|| "Failed to get metadata after successful upload")
    }

    ///
    /// Main method for downloading a file from the network. This method encapsulates the entire
    /// process of resolving the chunk storers, downloading the chunks, decoding them, and reassembling
    /// the file.
    ///
    /// Takes an input of the file metadata and the output directory where the file should be stored.
    ///
    /// Returns the path to the downloaded file.
    ///
    pub async fn download_file(
        &self,
        file_metadata: FileMetadata,
        output_dir: &Path,
    ) -> Result<PathBuf> {
        // Extract the list of chunk keys from the file metadata.
        let chunk_keys: Vec<Key> = file_metadata
            .chunks_metadata
            .iter()
            .map(|chunk| chunk.hash)
            .collect();

        // Resolve the nodes responsible for the given chunk keys.
        let resolved_pairs = self.find_storers_parallel(chunk_keys).await?;
        info!("File {} is downloadable", file_metadata.name);

        // Init file downloader
        let mut file_downloader = FileDownloader::new(file_metadata.clone(), output_dir).await?;

        let total_chunks = resolved_pairs.len();
        for (i, (chunk_id, chunk_storer)) in resolved_pairs.into_iter().enumerate() {
            // Download the chunk from the storer
            let chunk_data = self
                .download_chunk_from_storer(chunk_id, chunk_storer)
                .await?;
            info!("Chunk {} downloaded ({}/{})", chunk_id, i + 1, total_chunks);
            // Store the chunk in the file.
            file_downloader
                .store_next_chunk_decrypt(Chunk {
                    hash: chunk_id,
                    data: chunk_data,
                })
                .await?;
            info!("Chunk {} stored", chunk_id);
        }

        Ok(PathBuf::from(file_downloader.get_output_file_path()))
    }

    // ------------------------------------ HELPER METHODS --------------------------------- //

    ///
    /// General helper for sending a request to the specified node ID and awaiting the response.
    /// Abstracts the low-level details of sending a request and waiting for a response.
    ///
    /// This method should always be used instead of calling `send_request()` directly or manipulating
    /// the request map directly.
    ///
    /// The timeout should always be a global constant for specified action.
    ///
    async fn send_request_and_wait(
        &self,
        request: Request,
        timeout_milliseconds: u64,
    ) -> Result<Response, NodeError> {
        let request_id = request.request_id;

        // Create a one-shot channel that will get the response from the receiving loop
        let (sender, receiver) = oneshot::channel::<Response>();

        // Record the request in the request map
        self.request_map
            .add_request(request.request_id, sender)
            .await;

        // Send message, remove the request from the map if it fails
        if self.message_dispatcher.send_request(request).await.is_err() {
            self.request_map.remove_request(&request_id).await;
            return Err(NodeError::UnableToSend);
        }

        // Await the response from the channel, with given timeout.
        match timeout(Duration::from_millis(timeout_milliseconds), receiver).await {
            // Received something from the channel
            Ok(Ok(response)) => Ok(response),
            // The sender was dropped or otherwise no data came
            Ok(Err(_)) => Err(NodeError::BadResponse),
            // Timeout expired
            Err(_) => {
                // Remove the request from the map
                self.request_map.remove_request(&request_id).await;
                // "Timed out awaiting response."
                Err(NodeError::ResponseTimeout)
            }
        }
    }

    ///
    /// Method for actual data upload in parallel. This does
    /// connect to all nodes, which previously responded with
    /// ports, and does start an upload.
    ///
    async fn store_chunk(&self, ports: Vec<Response>, chunk: Chunk) -> Result<()> {
        let established_connection = ports
            .into_iter()
            .map(|node_with_port| self.send_chunk_over_tcp(node_with_port.clone(), chunk.clone()))
            .collect::<Vec<_>>();

        let sent_results = join_all(established_connection).await;

        // Partition results into successes and failures.
        let (successes, failures): (Vec<_>, Vec<_>) =
            sent_results.into_iter().partition(Result::is_ok);

        // Log each failure.
        failures.iter().for_each(|res| {
            if let Err(err) = res {
                warn!("Chunk replication upload to a node failed: {:?}", err);
            }
        });

        if !successes.is_empty() {
            if !failures.is_empty() {
                warn!(
            "Chunk upload: {} nodes failed while {} nodes succeeded. Note: this is fine, but the replication is not full.",
            failures.len(),
            successes.len()
        );
            }
            println!("Chunk successfully uploaded.");
            Ok(())
        } else {
            bail!("Chunk was not sent to any node!")
        }
    }

    ///
    /// Determines which responsive nodes are responsible for a
    /// chunk and does return ALPHA closest of them. The distance
    /// is calculated using the XOR metric.
    ///
    /// Performs a node lookup (similar to `find_node()`).
    ///
    async fn resolve_alpha_closest_nodes_to_key(&self, chunk_id: Key) -> Result<Vec<Response>> {
        let closest_nodes = self
            .find_node(chunk_id)
            .await
            .with_context(|| format!("Finding closest nodes for chunk {}", chunk_id))?;

        let store_request_futures = closest_nodes
            .into_iter()
            .map(|closest_node| self.send_initial_store_request(closest_node, chunk_id))
            .collect::<Vec<_>>();

        let mut responsive_nodes = join_all(store_request_futures)
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<Response>>();

        if responsive_nodes.is_empty() {
            bail!("Unable to store file. No responsive nodes.");
        }

        responsive_nodes.sort_by(|a, b| {
            let dist_a = a.clone().sender.id.distance(&chunk_id);
            let dist_b = b.clone().sender.id.distance(&chunk_id);
            dist_a.cmp(&dist_b)
        });

        // take ALPHA nodes with the smallest distance
        Ok(responsive_nodes.into_iter().take(ALPHA).collect())
    }

    ///
    /// This method returns a vector of nodes, which responded with `PORT`. That means
    /// that those are ready to accept a TCP data transfer.
    ///
    async fn get_ports_for_nodes(&self, nodes: Vec<Response>, key: Key) -> Result<Vec<Response>> {
        if nodes.is_empty() {
            bail!("No nodes to send the request to.");
        }

        if nodes.len() > ALPHA {
            bail!(
                "Too many nodes to send the request to - concurrency parameter ALPHA is {}",
                ALPHA
            );
        }

        // Prepare the futures for each node
        let nodes_with_ports = nodes
            .into_iter()
            .map(|closest_node| self.send_get_port_request(closest_node.sender, key))
            .collect::<Vec<_>>();

        // Nodes that are ready to accept TCP data transfer
        let responsive_nodes = join_all(nodes_with_ports)
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<Response>>();

        if responsive_nodes.is_empty() {
            bail!("Unable to store file. No nodes are ready to accept TCP data transfer.");
        }

        // take ALPHA nodes which XOR distance it the smallest
        Ok(responsive_nodes)
    }

    ///
    /// This method is a second step in the actual file upload process.
    /// It sends a `GET_PORT` request to a node and waits for a response.
    ///
    /// This request type is used to instruct the other node, that it has
    /// been selected for data transfer, and it should start listening on
    /// some port. The other node should expect a data transfer over TCP.
    ///
    /// The only accepted response is `PORT_OK` which means that the
    /// receiving node did provide a port for TCP data transfer.
    ///
    async fn send_get_port_request(&self, node_info: NodeInfo, key: Key) -> Option<Response> {
        let request = Request::new(
            RequestType::GetPort { chunk_id: key },
            self.to_node_info(),
            node_info,
        );

        match self
            .send_request_and_wait(request, BASIC_REQUEST_TIMEOUT_MILLISECONDS)
            .await
        {
            Ok(response) => {
                if let ResponseType::PortOK { .. } = response.response_type {
                    return Some(response);
                };
                None
            }
            _ => None,
        }
    }

    ///
    /// This method is a first step in the actual file upload process.
    /// It sends a `STORE` request to a node and waits for a response.
    ///
    /// If the response is of type STORE_OK, the node is ready to
    /// accept a data transfer. If the response is `STORE_CHUNK_UPDATED`
    /// the receiving node does already have the same chunk stored.
    /// In that case the receiving node does only update the TTL on
    /// that chunk, but is not ready to receive a chunk.
    ///
    async fn send_initial_store_request(
        &self,
        node_info: NodeInfo,
        file_id: Key,
    ) -> Option<Response> {
        let request = Request::new(
            RequestType::Store { chunk_id: file_id },
            self.to_node_info(),
            node_info,
        );

        // Just call the send_request_and_wait method, it will return err if the request fails
        match self
            .send_request_and_wait(request, BASIC_REQUEST_TIMEOUT_MILLISECONDS)
            .await
        {
            // Ready to accept data
            Ok(response) if response.response_type == ResponseType::StoreOK => {
                info!("Node '{}' is ready to accept data.", response.sender.id);
                Some(response)
            }
            // Updated storage, unable to accept
            Ok(response) if response.response_type == ResponseType::StoreChunkUpdated => {
                info!("Node '{}' updated the data.", response.sender.id);
                None
            }
            // Timed out or unable to accept
            _ => None,
        }
    }

    ///
    /// Method responsible for the physical chunk transfer using TCP.
    ///
    async fn send_chunk_over_tcp(&self, response: Response, chunk: Chunk) -> Result<()> {
        if let ResponseType::PortOK { port } = response.response_type {
            let tcp_address = format!("{}:{}", response.sender.address.ip(), port);
            let mut stream = TcpStream::connect(&tcp_address)
                .await
                .with_context(|| format!("Unable to connect to TCP stream at {}", tcp_address))?;

            stream
                .write_all(&chunk.data)
                .await
                .with_context(|| format!("Unable to write chunk data to {}", tcp_address))?;

            Ok(())
        } else {
            bail!("Unable to establish TCP stream.")
        }
    }

    ///
    /// Perform a single lookup to the given resolver node.
    ///
    /// This can be called individually, or used by `lookup_round()` for parallel calls.
    /// The type of resolution (Node/Value) is determined by the `for_value` parameter.
    ///
    async fn single_lookup(
        &self,
        target: Key,
        resolver_node_info: NodeInfo,
        for_value: bool,
    ) -> LookupResponse {
        let request_type = match for_value {
            true => RequestType::FindValue { chunk_id: target },
            false => RequestType::FindNode { node_id: target },
        };

        let request = Request::new(
            request_type.clone(),
            self.to_node_info(),
            resolver_node_info.clone(),
        );

        // Send out the request, await the response and parse it
        match self
            .send_request_and_wait(request, LOOKUP_TIMEOUT_MILLISECONDS)
            .await
        {
            // We received a response
            Ok(response) => {
                if let ResponseType::Nodes { nodes } = response.response_type {
                    LookupResponse::new_nodes_successful(resolver_node_info, nodes)
                }
                // Decide if the response is valid based on the context
                else if let ResponseType::HasValue { .. } = response.response_type {
                    return match for_value {
                        true => LookupResponse::new_value_successful(resolver_node_info),
                        false => LookupResponse::new_failed(resolver_node_info),
                    };
                }
                // Any other ResponseType is considered a failure
                else {
                    LookupResponse::new_failed(resolver_node_info)
                }
            }
            // If the request times out, return a failed response
            Err(_) => LookupResponse::new_failed(resolver_node_info),
        }
    }

    ///
    /// Sends requests to all the resolvers in parallel and awaits them all at a timeout.
    /// If the timeout is reached (per request), the returned `LookupResponse` is marked as false.
    ///
    /// - If resolvers is empty, returns an empty Vec.
    /// - Otherwise, spawns tasks for each resolver using `single_lookup()`.
    ///
    /// The type of resolution (Node/Value) is determined by the `for_value` parameter.
    ///
    async fn lookup_round(
        &self,
        target: Key,
        resolvers: Vec<NodeInfo>,
        for_value: bool,
    ) -> Vec<LookupResponse> {
        if resolvers.is_empty() {
            return Vec::new();
        }

        // For each resolver, create a future that calls single_lookup(...)
        let tasks = resolvers
            .into_iter()
            .map(|resolver_node_info| {
                let t_clone = target;
                self.single_lookup(t_clone, resolver_node_info, for_value)
            })
            .collect::<Vec<_>>();

        // Run all lookups in parallel
        join_all(tasks).await
    }

    ///
    /// This method resolves nodes responsible for the given chunk keys.
    /// The output can be used to download the chunks from the network.
    ///
    /// Performs the resolution in `ALPHA` parallel requests at a time.
    ///
    async fn find_storers_parallel(&self, chunk_keys: Vec<Key>) -> Result<Vec<(Key, NodeInfo)>> {
        let mut resolved_pairs = Vec::with_capacity(chunk_keys.len());

        // Process the chunk keys in batches of at most ALPHA.
        for batch in chunk_keys.chunks(ALPHA) {
            let batch_futures = batch.iter().map(|chunk_key| {
                let key = *chunk_key;
                async move {
                    // Resolve the storer for the given chunk key.
                    // If find_value() fails, this future will return an error.
                    let storer = self.find_storer(key).await?;
                    Ok::<(Key, NodeInfo), anyhow::Error>((key, storer))
                }
            });

            // Execute all futures in the batch concurrently.
            let batch_results = join_all(batch_futures).await;

            // Verify success or propagate error
            for res in batch_results {
                let pair = res?;
                resolved_pairs.push(pair);
            }
        }

        Ok(resolved_pairs)
    }

    ///
    /// Finds the first node in the network that is physically storing the chunk with the given key.
    ///
    /// If the chunk is not found, an error is returned.
    ///
    async fn find_storer(&self, target: Key) -> Result<NodeInfo> {
        // First step: Try to find _chunk locally
        if self
            .shard_storage_manager
            .read()
            .await
            .read_chunk(&target)
            .await
            .is_ok()
        // Ok is returned when the chunk is found on this node
        {
            return Ok(self.to_node_info());
        }

        // Initial resolvers are the alpha closest nodes to the target in our RT
        let initial_resolvers = self.routing_table.read().await.get_alpha_closest(&target)?;
        if initial_resolvers.is_empty() {
            bail!("No initial resolvers available for find_value resolutions");
        }

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(target, initial_resolvers, self.to_node_info());

        let mut current_responses = Vec::new();

        loop {
            // Get the resolvers for this
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
            if resolvers.is_empty() {
                break;
            }

            // Get the responses from all resolvers in this round
            current_responses = self.lookup_round(target, resolvers, true).await;

            // Record responses. If we didn't get any closer results OR we found the value, quit
            // this loop
            if !lookup_buffer.record_lookup_round_responses(current_responses.clone(), true) {
                break;
            }
        }

        // Check if we found the chunk storer
        if let Some(lookup_response) = current_responses
            .into_iter()
            .find(|response| matches!(response.get_success_type(), Some(LookupSuccessType::Value)))
        {
            let storer = lookup_response.get_resolver();
            info!("Found a node which stores chunk {}: {}", target, storer.id);
            return Ok(storer); // Finish early without exhaustive lookup
        }

        // If we didn't find the storer from the lookup rounds above, there is still possibility
        // that we didn't exhaust all the K closest nodes in the lookup buffer. Therefore, we
        // should try to resolve the storer from remaining unqueried nodes in the K closest we
        // have found so far.
        let remaining_resolvers = lookup_buffer.get_unqueried_resulting_nodes();
        // If we have exhausted all the K closest nodes, we can be sure that the chunk is not
        // stored in the network.
        if remaining_resolvers.is_empty() {
            bail!("Failed to find storer for chunk {}.", target);
        }
        // Finish up with the last lookup round
        let remaining_responses = self.lookup_round(target, remaining_resolvers, true).await;
        if let Some(lookup_response) = remaining_responses
            .into_iter()
            .find(|response| matches!(response.get_success_type(), Some(LookupSuccessType::Value)))
        {
            let storer = lookup_response.get_resolver();
            info!("Found a node which stores chunk {}: {}", target, storer.id);
            return Ok(storer);
        }

        // We are sure the chunk is not stored in the network.
        bail!("Failed to find storer for chunk {}.", target);
    }

    ///
    /// Downloads a single chunk given by the chunk_id from the storer node. This method expects the
    /// chunk to be available on the storer node.
    ///
    /// Returns an error if the chunk is not found on the provided storer node.
    ///
    async fn download_chunk_from_storer(
        &self,
        chunk_id: Key,
        chunk_storer: NodeInfo,
    ) -> Result<Vec<u8>> {
        // If we are the storer, we can read the chunk directly from the storage
        if chunk_storer.id == self.key {
            return self
                .shard_storage_manager
                .read()
                .await
                .read_chunk(&chunk_id)
                .await
                .with_context(|| format!("Failed to read chunk {} from local storage", chunk_id));
        }

        // Open a TCP Listener
        let tcp_listener_service = TcpListenerService::new()
            .await
            .with_context(|| "Failed to create TCP listener service for downloading chunk")?;

        // Send download request
        let request = Request::new(
            RequestType::GetValue {
                chunk_id,
                port: tcp_listener_service.get_port(),
            },
            self.to_node_info(),
            chunk_storer.clone(),
        );

        self.message_dispatcher
            .send_request(request)
            .await
            .with_context(|| {
                format!(
                    "Sending download request for chunk {} to {}",
                    chunk_id, chunk_storer
                )
            })?;

        // Receive data over TCP
        let data = tcp_listener_service.receive_data().await?;

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::messages::{RequestType, ResponseType};
    use std::error::Error;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_ping_response() -> Result<(), Box<dyn Error>> {
        let key = Key::new_random();

        let node = Arc::new(Node::new(key, "127.0.0.1".to_string(), 38102, "/".to_string()).await?);
        let node_address = node.address;

        let node_clone = Arc::clone(&node);
        // Start the listener so the node can receive requests.
        node_clone.start_listening();

        // Create a Ping request from a separate ephemeral socket.
        let ephemeral_socket = UdpSocket::bind("127.0.0.1:0").await?;
        let sender_info = NodeInfo::new(Key::new_random(), ephemeral_socket.local_addr().unwrap());

        // Create a Ping request
        let ping_request = Request::new(
            RequestType::Ping,
            sender_info.clone(), // Sender
            node.to_node_info(), // Receiver
        );

        // Send the Ping request to the node socket.
        let ping_request_bytes = serde_json::to_vec(&ping_request)
            .with_context(|| "Failed to serialize ping request")?;

        ephemeral_socket
            .send_to(&ping_request_bytes, node_address)
            .await?;

        // Wait for the nodes Pong response.
        let mut buffer = [0u8; 1024];
        let (size, _src) = timeout(
            Duration::from_secs(2),
            ephemeral_socket.recv_from(&mut buffer),
        )
        .await??;

        // Parse the received data and verify it is a Pong response.
        let response = serde_json::from_slice::<Response>(&buffer[..size])?;
        assert_eq!(response.response_type, ResponseType::Pong);

        // More assertions possible
        assert_eq!(response.request_id, ping_request.request_id);
        assert_eq!(response.receiver, sender_info);
        assert_eq!(response.sender, node.to_node_info());

        Ok(())
    }
}
