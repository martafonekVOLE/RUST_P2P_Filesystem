use crate::constants::PING_TIMEOUT_MILLISECONDS;
use crate::constants::{ALPHA, LOOKUP_TIMEOUT_MILLISECONDS};
use crate::core::incoming_request_handler::handle_received_request;
use crate::core::key::Key;
use crate::core::lookup::{LookupBuffer, LookupResponse};
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::networking::request_map::RequestMap;
use crate::routing::routing_table::RoutingTable;
use crate::storage::file_manager::{Chunk, FileManager};
use crate::storage::shard_storage_manager::ShardStorageManager;
use futures::future::join_all;
use log::info;
use std::net::SocketAddr;
use std::os::linux::raw::stat;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, UdpSocket as TokioUdpSocket};
use tokio::sync::{oneshot, RwLock};
use tokio::task;
use tokio::time::{timeout, Duration};

pub struct Node {
    pub(crate) key: Key,
    pub(crate) address: SocketAddr,
    socket: Arc<TokioUdpSocket>, // Use Tokio's UdpSocket for async I/O
    routing_table: Arc<RwLock<RoutingTable>>,
    request_map: RequestMap, // RequestMap is thread safe by design (has Arc<RwLock<HashMap>> inside)
    message_dispatcher: Arc<MessageDispatcher>,
    file_manager: FileManager,
    shard_storage_manager: Arc<RwLock<ShardStorageManager>>,
}

impl Node {
    pub async fn new(key: Key, ip: String, port: u16, storage_path: String) -> Self {
        let address: SocketAddr = format!("{}:{}", ip, port).parse().expect("Invalid address");
        let socket = TokioUdpSocket::bind(address)
            .await
            .expect("Failed to bind socket");
        let resolved_address = socket
            .local_addr()
            .expect("Failed to get local socket address");
        Node {
            key,
            address: resolved_address,
            socket: Arc::new(socket),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(key))),
            request_map: RequestMap::new(),
            message_dispatcher: Arc::new(MessageDispatcher::new().await),
            file_manager: FileManager::new(),
            shard_storage_manager: Arc::new(RwLock::new(ShardStorageManager::new(storage_path))),
        }
    }

    ///
    /// This method converts Node to NodeInfo
    ///
    fn to_node_info(&self) -> NodeInfo {
        NodeInfo::new(self.key, self.address)
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
    ) -> Result<Response, &'static str> {
        let request_id = request.request_id;

        // Create a one-shot channel that will get the response from the receiving loop
        let (sender, receiver) = oneshot::channel::<Response>();

        // Record the request in the request map
        self.request_map
            .add_request(request.request_id, sender)
            .await;

        // Send message
        if let Err(_send_err) = self.message_dispatcher.send_request(request).await {
            return Err("Unable to send request via dispatcher.");
        }

        // Await the response from the channel, with given timeout.
        match timeout(Duration::from_millis(timeout_milliseconds), receiver).await {
            // Received something from the channel
            Ok(Ok(response)) => Ok(response),
            // The sender was dropped or otherwise no data came
            Ok(Err(_)) => Err("No response received on the channel."),
            // Timeout expired
            Err(_) => {
                // Remove the request from the map
                self.request_map.remove_request(&request_id).await;
                Err("Timed out awaiting response.")
            }
        }
    }

    ///
    /// Sends a PING request to the specified node ID.
    /// Awaits a PONG response and returns whether the node is reachable.
    ///
    pub async fn ping(&self, node_id: Key) -> Result<Response, &str> {
        // Get the address from the RoutingTable
        let receiver_info = match self.routing_table.read().await.get_nodeinfo(&node_id) {
            Some(info) => info.clone(),
            None => return Err("PING failed. Node not in routing table."),
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
    pub async fn find_node(&self, target: Key) -> Result<Vec<NodeInfo>, String> {
        // Initial resolvers are the alpha closest nodes to the target in our RT
        let initial_resolvers = self.routing_table.read().await.get_alpha_closest(&target)?;
        if initial_resolvers.is_empty() {
            return Err("No initial resolvers available for find_node resolutions".to_string());
        }

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(target.clone(), initial_resolvers);

        loop {
            // Get the resolvers for this
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
            if resolvers.is_empty() {
                break;
            }

            // Get the responses from all resolvers in this round
            let current_responses = self.lookup_round(target, resolvers).await;

            // Record responses. If we didn't get any closer results, quit the rounds loop
            if !lookup_buffer.record_lookup_round_responses(current_responses) {
                break;
            }
        }

        // Execute the last lookup on all the resulting nodes that haven't been queried yet
        let last_lookup_responses = self
            .lookup_round(target, lookup_buffer.get_unqueried_resulting_nodes())
            .await;
        lookup_buffer.record_lookup_round_responses(last_lookup_responses);

        // Update routing table with the responsive nodes
        let mut routing_table = self.routing_table.write().await;
        for node_info in lookup_buffer.get_responsive_nodes() {
            routing_table.store_nodeinfo(node_info);
        }

        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// Method handling file uploads
    ///
    pub async fn store(&self, file_path: &str) -> Result<(), &str> {
        // Step 1: check file existence
        if !self.file_manager.check_file_exists(file_path) {
            return Err("File not found.");
        }

        // Step 2: call start sharding

        // Step 3: loop over shards
        loop {
            // Step 3.1: get next shard (todo change this stub into "next")
            let chunk = FileManager::temp_sharding();

            match chunk {
                Some(chunk) => {
                    // Step 4: get ALPHA nodes responsible for the chunk
                    let responsible_nodes = self
                        .get_responsive_nodes_responsible_for_chunk(chunk.clone())
                        .await?;

                    // Step 5: get ports for TCP data transfer
                    let ports = self
                        .request_available_port_for_tcp_data_transfer(
                            responsible_nodes,
                            chunk.clone().get_hash(),
                        )
                        .await?;

                    let mut successfully_sent_to: Vec<NodeInfo> = Vec::new();

                    // Step 6: send data to each node over TCP
                    for response in ports {
                        let status = self
                            .establish_tcp_stream_and_send_data(
                                response.clone(),
                                chunk.clone().get_data(),
                            )
                            .await;
                        if let Ok(_) = status {
                            // Step 7: save info to FileManager
                            self.file_manager.save_data_sent(
                                &response.sender,
                                chunk.clone().get_hash(),
                                SystemTime::now(),
                            );
                            successfully_sent_to.push(response.sender.clone())
                        }
                    }

                    if successfully_sent_to.len() > 0 {
                        println!("Chunk successfully uploaded.");
                    } else {
                        // Future-improvement: Chunk may be sent again to different nodes.
                        return Err("Chunk was not sent!");
                    }
                }
                _ => {
                    // Step 8: Close file descriptor
                    return Ok(());
                }
            }
        }
    }

    ///
    /// Get ALPHA most responsive closest nodes for provided chunk.
    ///
    async fn get_responsive_nodes_responsible_for_chunk(
        &self,
        chunk: Chunk,
    ) -> Result<Vec<Response>, &str> {
        let closest_nodes = self
            .find_node(chunk.get_hash())
            .await
            .expect("No closest nodes.");
        let mut responsive_alpha_closest_nodes: Vec<Response> = Vec::new();

        // Step 4.1: send STORE request to K nodes
        for node in closest_nodes {
            // Future-improvement: we can update K-bucket with responsive nodes or remove ones which did not respond
            if let Ok(response) = self.send_initial_store_request(node).await {
                if response.response_type == ResponseType::StoreOK {
                    // Step 4.2: take ALPHA responsive nodes (future-improvement: take fastest)
                    responsive_alpha_closest_nodes.push(response);
                }
            }

            if responsive_alpha_closest_nodes.len() >= ALPHA {
                break;
            }
        }

        if responsive_alpha_closest_nodes.len() == 0 {
            return Err("No responsive nodes.");
        }

        Ok(responsive_alpha_closest_nodes)
    }

    ///
    /// Receive ports for establishing TCP connection
    ///
    async fn request_available_port_for_tcp_data_transfer(
        &self,
        nodes: Vec<Response>,
        key: Key,
    ) -> Result<Vec<Response>, &str> {
        let mut ports_responses: Vec<Response> = Vec::new();
        for response in nodes {
            if let Ok(response) = self
                .send_store_port_request(response.sender, key.clone())
                .await
            {
                if let (ResponseType::StorePortOK { port }) = response.response_type {
                    ports_responses.push(response);
                }
            }
        }

        if ports_responses.len() == 0 {
            return Err("No port returned from nodes.");
        }

        Ok(ports_responses)
    }

    ///
    /// Send initial STORE request to a node
    ///
    async fn send_initial_store_request(&self, node_info: NodeInfo) -> Result<Response, &str> {
        let request = Request::new(RequestType::Store, self.to_node_info(), node_info);

        // Just call the send_request_and_wait method, it will return err if the request fails
        self.send_request_and_wait(request, PING_TIMEOUT_MILLISECONDS)
            .await
    }

    ///
    /// Send STORE PORT request
    ///
    async fn send_store_port_request(
        &self,
        node_info: NodeInfo,
        key: Key,
    ) -> Result<Response, &str> {
        let request = Request::new(
            RequestType::StorePort { file_id: key },
            self.to_node_info(),
            node_info,
        );

        self.send_request_and_wait(request, PING_TIMEOUT_MILLISECONDS)
            .await
    }

    ///
    /// Send file over TCP
    ///
    async fn establish_tcp_stream_and_send_data(
        &self,
        response: Response,
        data: Vec<u8>,
    ) -> Result<(), &str> {
        if let (ResponseType::StorePortOK { port }) = response.response_type {
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
            Err("Unable to establish TCP stream.")
        }
    }

    ///
    /// Performs the join network procedure to join an existing network.
    /// This method returns Err if the beacon node does not respond.
    /// After successfully joining the network, the responsive nodes are stored in the routing table
    /// and the Node is ready to participate in the network.
    ///
    pub async fn join_network(&self, beacon_node: NodeInfo) -> Result<Vec<NodeInfo>, String> {
        self.routing_table
            .write()
            .await
            .store_nodeinfo(beacon_node.clone());

        info!("Joining network with beacon node: {}", beacon_node.id);
        self.ping(beacon_node.id).await?;

        // Send single find_node to the beacon to get the initial k-closest nodes
        let initial_k_closest = self
            .single_lookup(self.key.clone(), beacon_node.clone())
            .await
            .get_k_closest();

        if initial_k_closest.is_empty() {
            return Err("Failed to join network. Beacon node did not respond.".to_string());
        }

        // Inject the initial resolvers into the result buffer
        let mut lookup_buffer = LookupBuffer::new(self.key.clone(), initial_k_closest);

        // Execute all lookup rounds
        loop {
            // Get the resolvers for the next round
            let resolvers = lookup_buffer.get_alpha_unqueried_nodes();
            if resolvers.is_empty() {
                break;
            }

            // Get the responses from all resolvers in this round
            let current_responses = self.lookup_round(self.key, resolvers).await;

            // Record responses. If we didn't get any closer results, quit the rounds loop
            if !lookup_buffer.record_lookup_round_responses(current_responses) {
                break;
            }
        }

        // Execute the last lookup on all the resulting nodes that haven't been queried yet
        let last_lookup_responses = self
            .lookup_round(self.key, lookup_buffer.get_unqueried_resulting_nodes())
            .await;
        lookup_buffer.record_lookup_round_responses(last_lookup_responses);

        // Update routing table with the responsive nodes
        let mut routing_table = self.routing_table.write().await;
        for node_info in lookup_buffer.get_responsive_nodes() {
            routing_table.store_nodeinfo(node_info);
        }

        // Render the final result
        Ok(lookup_buffer.get_resulting_vector())
    }

    ///
    /// Perform a single lookup to the given resolver node.
    /// This can be called individually, or used by lookup_round for parallel calls.
    ///
    pub async fn single_lookup(&self, target: Key, resolver_node_info: NodeInfo) -> LookupResponse {
        let request = Request::new(
            RequestType::FindNode { node_id: target },
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
                // If the response is a Nodes response, return a successful LookupResponse
                if let ResponseType::Nodes { nodes } = response.response_type {
                    LookupResponse::new_successful(resolver_node_info, nodes)
                // Any other ResponseType is considered a failure
                } else {
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
    pub async fn lookup_round(&self, target: Key, resolvers: Vec<NodeInfo>) -> Vec<LookupResponse> {
        if resolvers.is_empty() {
            return Vec::new();
        }

        // For each resolver, create a future that calls single_lookup(...)
        let tasks = resolvers
            .into_iter()
            .map(|resolver_node_info| {
                let t_clone = target.clone();
                self.single_lookup(t_clone, resolver_node_info)
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
    pub fn start_listening(&self) {
        // Load references to the fields of the Node struct
        let socket = Arc::clone(&self.socket);
        let routing_table = Arc::clone(&self.routing_table);
        let request_map = self.request_map.clone();
        let message_dispatcher = Arc::clone(&self.message_dispatcher);
        let shard_manager = Arc::clone(&self.shard_storage_manager);
        let this_node_info = self.to_node_info();

        // Spawn an indefinitely looping async task to listen for incoming messages
        task::spawn(async move {
            let mut buffer = [0; 1024]; // FIXME: Magic number
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, src)) => {
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
                            task::spawn(async move {
                                handle_received_request(
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
                            eprintln!("Failed to parse message from {}", src);
                        }
                    }
                    // Reading from the socket failed
                    Err(e) => {
                        eprintln!("Error receiving message: {}", e);
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::networking::messages::{RequestType, ResponseType};
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn test_ping_response() -> Result<(), Box<dyn std::error::Error>> {
        let key = Key::new_random();
        let node = Node::new(key.clone(), "127.0.0.1".to_string(), 8081, "/".to_string()).await;
        let node_address = node.address;

        // Start the listener so the node can receive requests.
        node.start_listening();

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
        ephemeral_socket
            .send_to(&ping_request.to_bytes(), node_address)
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
