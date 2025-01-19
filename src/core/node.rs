use crate::config::ALPHA;
use crate::core::incoming_request_handler::handle_received_request;
use crate::core::key::{Key, KeyValue};
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response, ResponseType};
use crate::networking::node_info::NodeInfo;
use crate::networking::request_map::RequestMap;
use crate::routing::routing_table::RoutingTable;
use futures::future::join_all;
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::oneshot::Receiver;
use tokio::sync::{oneshot, RwLock};
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::{timeout, Duration};

pub struct Node {
    key: Key,
    address: SocketAddr,
    socket: Arc<TokioUdpSocket>, // Use Tokio's UdpSocket for async I/O
    routing_table: Arc<RwLock<RoutingTable>>,
    request_map: RequestMap, // RequestMap is thread safe by design (has Arc<RwLock<HashMap>> inside)
    message_dispatcher: Arc<MessageDispatcher>,
}

struct LookupResult<'a> {
    closest: KeyValue,
    response: Vec<Result<Response, &'a str>>,
}

impl LookupResult<'_> {
    pub fn get_closest(&self) -> KeyValue {
        self.closest.clone()
    }

    pub fn get_response(&self) -> Vec<Result<Response, &str>> {
        self.response.clone()
    }
}

impl Node {
    pub async fn new(
        key: Key,
        ip: String,
        port: u16,
        bucket_size: usize,
        num_buckets: usize,
    ) -> Self {
        let address = format!("{}:{}", ip, port).parse().expect("Invalid address");
        let socket = TokioUdpSocket::bind(address)
            .await
            .expect("Failed to bind socket");

        Node {
            key,
            address,
            socket: Arc::new(socket),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(key))),
            request_map: RequestMap::new(),
            message_dispatcher: Arc::new(MessageDispatcher::new().await),
        }
    }

    ///
    /// This method converts Node to NodeInfo
    ///
    fn to_node_info(&self) -> NodeInfo {
        NodeInfo::new(self.key, self.address)
    }

    ///
    /// Sends a PING request to the specified node ID.
    /// Awaits a PONG response and returns whether the node is reachable.
    ///
    pub async fn ping(&self, node_id: Key) -> Result<Response, &str> {
        // Get the address from RT
        let receiver = match self.routing_table.read().await.get_nodeinfo(&node_id) {
            Some(receiver) => receiver.clone(),
            None => return Err("PING failed. Receiver could not be found in the routing table."),
        };

        // Create request
        let request = Request::new(RequestType::Ping, self.to_node_info(), receiver);

        // Create comms channel
        let (sender, receiver) = oneshot::channel::<Response>();

        // Record request in the request map
        self.request_map
            .add_request(request.request_id, sender)
            .await;

        // Use message dispatcher send
        let _dispatcher_response = match self.message_dispatcher.send_request(request).await {
            Ok(response) => response,
            Err(_) => return Err("PING failed. Unable to send request."),
        };

        match timeout(Duration::from_secs(10), receiver).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err("PING failed. Did not receive any response."),
            Err(_) => Err("PING failed. Timed out."),
        }
    }

    ///
    /// Sends a FIND_NODE request to the specified node ID.
    /// Awaits a response containing the closest nodes to the requested node ID.
    ///
    pub async fn find_node(&self, node_id: Key) -> Result<NodeInfo, &str> {
        // If I know the node, I can immediately return the result
        if let Some(node_info) = self.routing_table.read().await.get_nodeinfo(&node_id) {
            return Ok(node_info.clone());
        }

        // Get all of my known nodes
        let routing_table = self.routing_table.read().await;
        let mut nodes = routing_table.get_all_nodeinfos();

        loop {
            // If I have no more nodes to iterate over, the lookup ends without a success
            // This is empty when Node does not have any other nodes in its RT or parsing found
            // nodes did not return any new closer nodes.
            if nodes.is_empty() {
                return Err("FIND NODE failed. No nodes to lookup.");
            }

            // Pass the nodes to a method which will send a request to them
            let lookup_result = self.lookup_through_nodes(nodes, node_id).await?;
            let lookup_response = lookup_result.get_response();
            let lookup_closest_node = lookup_result.closest;

            let found_nodes =
                Self::parse_found_nodes(lookup_response, lookup_closest_node, node_id);
            if found_nodes.len() == 1 {
                if let Some(looked_up_node) = found_nodes.get(&node_id) {
                    return Ok(looked_up_node.clone());
                }
            }

            if found_nodes.is_empty() {
                return Err("FIND NODE failed. Could not found the node.");
            }

            nodes = found_nodes.values().cloned().collect();
        }
    }

    ///
    ///
    ///
    async fn lookup_through_nodes(
        &self,
        nodes: Vec<NodeInfo>,
        node_id: Key,
    ) -> Result<LookupResult, &str> {
        let mut sorted_nodes = nodes.clone();
        sorted_nodes.sort_by(|a: &NodeInfo, b: &NodeInfo| {
            let dist_a = a.id.distance(&node_id);
            let dist_b = b.id.distance(&node_id);
            dist_a.cmp(&dist_b)
        });

        let closest_node = match sorted_nodes.first() {
            Some(node) => node.id.distance(&node_id),
            None => return Err("FIND NODE failed. Unable to update the closest node."), // todo maybe we should initiate?
        };

        // Based on this: https://www.syncfusion.com/succinctly-free-ebooks/kademlia-protocol-succinctly/node-lookup
        // We only care about the initial (alpha closest) nodes
        let (initial_nodes, other_nodes) = sorted_nodes.split_at(ALPHA);

        // Initial alpha responses
        let responses = join_all(
            self.dispatch_find_nodes_async(initial_nodes.to_vec(), node_id)
                .await,
        )
        .await;
        let successful_responses: Vec<_> = responses
            .into_iter()
            .filter_map(|response| response.ok())
            .collect();

        if successful_responses.is_empty() {
            return Err("FIND NODES failed. All requests failed.");
        };

        Ok(LookupResult {
            closest: closest_node,
            response: successful_responses,
        })
    }

    fn parse_found_nodes(
        successful_responses: Vec<Result<Response, &str>>,
        closest_node: KeyValue,
        node_id: Key,
    ) -> HashMap<Key, NodeInfo> {
        let mut new_nodes: HashMap<Key, NodeInfo> = HashMap::new();

        for response_result in successful_responses {
            match response_result {
                Ok(response) => {
                    if let ResponseType::Nodes { nodes } = response.response_type {
                        for node in nodes.iter().flatten() {
                            // Found node, return
                            if node.id == node_id {
                                new_nodes.clear();
                                new_nodes.insert(node.id.clone(), node.clone());
                                return new_nodes;
                            }

                            // Found closer node, add to closer nodes
                            if node.id.distance(&node_id) < closest_node {
                                new_nodes.insert(node.id.clone(), node.clone());
                            }
                        }
                    }
                }
                _ => continue,
            }
        }

        new_nodes
    }

    async fn dispatch_find_nodes_async(
        &self,
        sorted_nodes: Vec<NodeInfo>,
        node_id: Key,
    ) -> Vec<JoinHandle<Result<Response, &str>>> {
        let mut requests = vec![];

        for node in sorted_nodes.iter() {
            let request = Request::new(
                RequestType::FindNode { node_id },
                self.to_node_info(),
                node.clone(),
            );

            let (sender, receiver) = oneshot::channel::<Response>();

            self.request_map
                .add_request(request.request_id, sender)
                .await;

            let messenger = self.message_dispatcher.clone();

            let task = tokio::spawn(async move {
                let _dispatcher_response = match messenger.send_request(request).await {
                    Ok(response) => response,
                    Err(_) => return Err("FIND NODE failed. Unable to send request."),
                };

                match timeout(Duration::from_secs(10), receiver).await {
                    Ok(Ok(response)) => Ok(response),
                    Ok(Err(_)) => Err("FIND NODE failed. Did not receive any response."),
                    Err(_) => Err("FIND NODE failed. Timed out."),
                }
            });

            requests.push(task);
        }

        requests
    }

    pub fn join_network() {
        todo!()
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
                            task::spawn(async move {
                                handle_received_request(request, routing_table, message_dispatcher)
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
