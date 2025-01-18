use crate::core::incoming_request_handler::handle_received_request;
use crate::core::key::Key;
use crate::networking::message_dispatcher::MessageDispatcher;
use crate::networking::messages::{Request, RequestType, Response};
use crate::networking::node_info::NodeInfo;
use crate::networking::request_map::RequestMap;
use crate::routing::routing_table::RoutingTable;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::{oneshot, RwLock};
use tokio::task;
use tokio::time::{timeout, Duration};

pub struct Node {
    key: Key,
    address: SocketAddr,
    node_info: NodeInfo,
    socket: Arc<TokioUdpSocket>, // Use Tokio's UdpSocket for async I/O
    routing_table: Arc<RwLock<RoutingTable>>,
    request_map: RequestMap, // RequestMap is thread safe by design (has Arc<RwLock<HashMap>> inside)
    message_dispatcher: Arc<MessageDispatcher>,
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

        let this_node_info = NodeInfo {
            id: key,
            address,
        };

        Node {
            key,
            address,
            node_info: this_node_info,
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
            Some(receiver) => receiver,
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
    pub async fn find_node(&self, node_id: Key) -> Option<Vec<NodeInfo>> {
        todo!()
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
