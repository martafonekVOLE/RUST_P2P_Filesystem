use crate::constants::PING_TIMEOUT_MILLISECONDS;
use crate::constants::{ALPHA, LOOKUP_TIMEOUT_MILLISECONDS};
use crate::core::incoming_request_handler::handle_received_request;
use crate::core::key::Key;
use crate::core::lookup::{LookupBuffer, LookupResponse, LookupSuccessType};
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
use log::{error, info};
use std::cmp::PartialEq;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, UdpSocket as TokioUdpSocket};
use tokio::sync::{oneshot, RwLock};
use tokio::task;
use tokio::time::{timeout, Duration};

use anyhow::{bail, Context, Result};

use crate::networking::tcp_listener::TcpListenerService;
use crate::sharding::downloader::FileDownloader;
use thiserror::Error;

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
    pub key: Key,
    pub address: SocketAddr,
    socket: Arc<TokioUdpSocket>, // Use Tokio's UdpSocket for async I/O
    pub routing_table: Arc<RwLock<RoutingTable>>,
    request_map: RequestMap, // RequestMap is thread safe by design (has Arc<RwLock<HashMap>> inside)
    message_dispatcher: Arc<MessageDispatcher>,
    file_manager: Arc<FileManager>,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
}

impl Node {
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
    /// This method converts Node to NodeInfo
    ///
    pub fn to_node_info(&self) -> NodeInfo {
        NodeInfo::new(self.key, self.address)
    }

    ///
    /// Dumps the routing table into a string form
    ///
    pub async fn get_routing_table_content(&self) -> Vec<NodeInfo> {
        self.routing_table.read().await.get_all_nodeinfos()
    }

    ///
    /// Returns a vector of all chunks this node stores
    ///
    pub async fn get_owned_chunk_keys(&self) -> Vec<Key> {
        self.shard_storage_manager
            .read()
            .await
            .get_owned_chunk_keys()
    }

    ///
    /// Sends a request to the specified node ID and awaits the response.
    /// This method should always be used instead of calling send_request directly or manipulating
    /// the request map directly.
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

        // Send message
        self.message_dispatcher
            .send_request(request)
            .await
            .map_err(|_| NodeError::UnableToSend)?;

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
        self.send_request_and_wait(request, PING_TIMEOUT_MILLISECONDS)
            .await
    }

    ///
    /// Attempts to find the closest K nodes in the entire network with respect to the target key.
    /// This method can only be used if the node is already part of the network. (the node already
    /// successfully underwent the join_network procedure)
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
            routing_table.store_nodeinfo(node_info, &self);
        }

        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// Method handling file uploads
    ///
    pub async fn upload_file(&self, file_path: &str) -> Result<FileMetadata> {
        // Step 1: check file existence
        if !Path::new(file_path).exists() {
            bail!(
                "Can't upload nonexistent file '{}'. Did you enter the correct path?",
                file_path
            );
        }

        // Step 2: call start sharding
        let mut uploader = FileUploader::new(file_path).await?;

        // Step 3: loop over shards
        loop {
            // Step 3.1: get next shard
            let chunk = uploader.get_next_chunk_encrypt().await?;

            match chunk {
                Some(chunk) => {
                    // Step 4: get the ALPHA closest nodes responsible for the chunk
                    let responsible_nodes = self
                        .get_alpha_closest_nodes_to_key(chunk.clone().hash)
                        .await?;

                    // Step 5: get ports for TCP data transfer
                    let mut ports = self
                        .get_ports_for_nodes(responsible_nodes, chunk.clone().hash)
                        .await?;

                    if ports.is_empty() {
                        bail!("No port returned from nodes.");
                    }

                    // Step 6: send data to each node over TCP
                    let established_connection = ports
                        .into_iter()
                        .map(|node_with_port| {
                            self.establish_tcp_stream_and_send_data(
                                node_with_port.clone(),
                                chunk.clone().data,
                            )
                        })
                        .collect::<Vec<_>>();

                    // vec <result <()>>
                    let sent_to_nodes = join_all(established_connection).await;

                    // Check if at least one result is Ok
                    if sent_to_nodes.iter().any(|result| result.is_ok()) {
                        info!("Chunk {} successfully uploaded.", chunk.hash);
                    } else {
                        // Future-improvement: Chunk may be sent again to different nodes.
                        bail!("Chunk {} could not be sent!", chunk.hash);
                    }
                }
                None => {
                    break;
                }
            }
        }

        // Step 7: save info to FileManager
        self.file_manager.add_file_uploaded_entry(file_path).await;

        // Get the file metadata and return it
        uploader
            .get_metadata()
            .await
            .with_context(|| "Failed to get metadata after successful upload")
    }

    ///
    /// Get ALPHA most responsive closest nodes for provided chunk.
    ///
    async fn get_alpha_closest_nodes_to_key(&self, chunk_id: Key) -> Result<Vec<Response>> {
        let closest_nodes = self
            .find_node(chunk_id)
            .await
            .with_context(|| format!("Finding closest nodes for chunk {}", chunk_id))?;

        let store_request_futures = closest_nodes
            .into_iter()
            .map(|closest_node| self.send_initial_store_request(closest_node, chunk_id))
            .collect::<Vec<_>>();

        let mut responsive_nodes = join_all(store_request_futures).await;

        if responsive_nodes.is_empty() {
            bail!("Unable to store file. No responsive nodes.");
        }

        responsive_nodes.sort_by(|a, b| {
            // Unwrap can be used here, because get_responsive_nodes_round does return
            // only valid responses as Some(response)
            let dist_a = a.clone().unwrap().sender.id.distance(&chunk_id);
            let dist_b = b.clone().unwrap().sender.id.distance(&chunk_id);
            dist_a.cmp(&dist_b)
        });

        // take ALPHA nodes where XOR distance is the smallest
        let alpha_nodes = responsive_nodes.into_iter().flatten().take(ALPHA).collect();

        Ok(alpha_nodes)
    }

    ///
    /// This method returns a vector of nodes, which responded with PORT. That means
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
            .map(|closest_node| self.send_store_port_request(closest_node.sender, key))
            .collect::<Vec<_>>();

        // Nodes that are ready to accept TCP data transfer
        let mut responsive_nodes = join_all(nodes_with_ports).await;

        if responsive_nodes.is_empty() {
            bail!("Unable to store file. No nodes are ready to accept TCP data transfer.");
        }

        // take ALPHA nodes which XOR distance it the smallest
        let flatten = responsive_nodes.into_iter().flatten().collect();
        Ok(flatten)
    }

    ///
    /// Send STORE PORT request
    ///
    async fn send_store_port_request(&self, node_info: NodeInfo, key: Key) -> Option<Response> {
        let request = Request::new(
            RequestType::GetPort { chunk_id: key },
            self.to_node_info(),
            node_info,
        );

        match self
            .send_request_and_wait(request, PING_TIMEOUT_MILLISECONDS)
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
    /// Send initial STORE request to a node
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
            .send_request_and_wait(request, PING_TIMEOUT_MILLISECONDS) // FIXME Specify timeout
            .await
        {
            Ok(response) if response.response_type == ResponseType::StoreOK => {
                info!("Node '{}' is ready to accept data.", response.sender.id);
                Some(response)
            }
            Ok(response) if response.response_type == ResponseType::StoreChunkUpdated => {
                info!("Node '{}' updated the data.", response.sender.id);
                None
            }
            _ => None,
        }
    }

    ///
    /// Send file over TCP
    ///
    async fn establish_tcp_stream_and_send_data(
        &self,
        response: Response,
        data: Vec<u8>,
    ) -> Result<()> {
        if let ResponseType::PortOK { port } = response.response_type {
            let address =
                response.sender.address.ip().to_string() + ":" + port.to_string().as_str();
            let mut stream = TcpStream::connect(address)
                .await
                .expect("Unable to connect to the TCP Stream.");

            stream
                .write_all(data.as_slice())
                .await
                .expect("Unable to write data.");

            Ok(())
        } else {
            bail!("Unable to establish TCP stream.")
        }
    }

    ///
    /// Performs the join network procedure to join an existing network.
    /// This method returns Err if the beacon node does not respond.
    /// After successfully joining the network, the responsive nodes are stored in the routing table
    /// and the Node is ready to participate in the network.
    ///
    pub async fn join_network(&self, beacon_node: NodeInfo) -> Result<Vec<NodeInfo>> {
        info!(
            "Beginning join_network procedure via beacon {}",
            beacon_node.id
        );
        self.routing_table
            .write()
            .await
            .store_nodeinfo(beacon_node.clone(), &self)
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
        initial_k_closest.retain(|node| *node != self.to_node_info());

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(self.key, initial_k_closest, self.to_node_info());

        // Execute all lookup rounds
        loop {
            // Get the resolvers for the next round
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
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
            routing_table.store_nodeinfo(node_info, &self).await?;
        }

        info!("Successfully joined the network!");
        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// Perform a single lookup to the given resolver node.
    /// This can be called individually, or used by lookup_round for parallel calls.
    /// The type of resolution (Node/Value) is determined by the for_value parameter.
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
                if let ResponseType::Nodes { node_infos: nodes } = response.response_type {
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
    /// - Otherwise, spawns tasks for each resolver using `single_lookup`.
    ///
    /// The type of resolution (Node/Value) is determined by the for_value parameter.
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
    /// Creates a listening task that indefinitely waits for incoming messages on this node's socket.
    /// Incoming messages are parsed and dispatched to the appropriate handler.
    /// This method is non-blocking and runs in the background.
    ///
    pub fn start_listening(self: Arc<Self>) {
        // Load references to the fields of the Node struct
        let socket = Arc::clone(&self.socket);
        let routing_table = Arc::clone(&self.routing_table);
        let request_map = self.request_map.clone();
        let message_dispatcher = Arc::clone(&self.message_dispatcher);
        let shard_manager = Arc::clone(&self.shard_storage_manager);
        let this_node_info = self.to_node_info();

        // Spawn an indefinitely looping async task to listen for incoming messages
        task::spawn(async move {
            let mut buffer = [0; MAX_MESSAGE_SIZE];
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, src)) => {
                        //bp here
                        let data = &buffer[..size];

                        // Response
                        if let Ok(response) = serde_json::from_slice::<Response>(data) {
                            let request_map = request_map.clone();
                            task::spawn(async move {
                                // Use the map to send the response into the corresponding oneshot
                                // Unknown requests IDs are filtered out by this method
                                request_map.handle_response(response).await;
                            });

                        // Request
                        } else if let Ok(request) = serde_json::from_slice::<Request>(data) {
                            let routing_table = Arc::clone(&routing_table);
                            let message_dispatcher = Arc::clone(&message_dispatcher);
                            let shard_manager = Arc::clone(&shard_manager);
                            let this_node_info = this_node_info.clone();
                            let this = self.clone();
                            task::spawn(async move {
                                handle_received_request(
                                    this,
                                    this_node_info,
                                    request,
                                    routing_table,
                                    message_dispatcher,
                                    shard_manager,
                                )
                                .await;
                            });

                        // Invalid incoming message
                        } else {
                            error!("Failed to parse message from {}", src);
                        }
                    }
                    // Reading from the socket failed
                    Err(e) => {
                        // bp here
                        error!("Error receiving message: {}", e);
                    }
                }
            }
        });
    }

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

        for (i, (chunk_id, chunk_storer)) in resolved_pairs.into_iter().enumerate() {
            // Download the chunk from the storer
            let chunk_data = self
                .download_chunk_from_storer(chunk_id, chunk_storer)
                .await?;
            info!(
                "Chunk {} downloaded (number {} from {})",
                chunk_id,
                i + 1,
                chunk_data.len()
            );
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

    ///
    /// This method resolves nodes responsible for the given chunk keys.
    /// The output can be used to download the chunks from the network.
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

            // Record responses. If we didn't get any closer results, quit the rounds loop
            if !lookup_buffer.record_lookup_round_responses(current_responses.clone(), true) {
                break;
            }

            // FIXME this doesn't exhaust all K closest nodes to the value (see find_node)
        }

        // Check if we found the _chunk storer
        if let Some(lookup_response) = current_responses
            .into_iter()
            .find(|response| matches!(response.get_success_type(), Some(LookupSuccessType::Value)))
        {
            let storer = lookup_response.get_resolver();
            info!("Found a node which stores chunk {}: {}", target, storer.id);
            return Ok(storer);
        }

        bail!("Failed to find storer for chunk {}.", target);
    }

    ///
    /// Downloads a chunk from the storer node. This method expects the chunk to be available
    /// on the storer node and will return an error if the chunk is not found.
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
        let node = Arc::new(Node::new(key, "127.0.0.1".to_string(), 8081, "/".to_string()).await?);
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
