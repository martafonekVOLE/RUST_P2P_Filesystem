use crate::core::key::Key;
use crate::networking::node_service::handle_received_request;
use crate::routing::kademlia_messages;
use crate::routing::routing_table::RoutingTable;
use sha1::Digest;
use std::borrow::Cow;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct Node {
    key: Key,
    address: SocketAddr,
    socket: UdpSocket,
    routing_table: Arc<RwLock<RoutingTable>>,
    request_map: Arc<RwLock<HashMap<RequestId, oneshot::Sender<Message>>>>
}

impl Node {
    pub fn new(key: Key, ip: String, port: u16, bucket_size: usize, num_buckets: usize) -> Self {
        let address = format!("{}:{}", ip, port).parse().expect("Invalid address");
        let socket = UdpSocket::bind(address).expect("Failed to bind socket");
        Node {
            key,
            address,
            routing_table: Arc::new(RwLock::new(RoutingTable::new(bucket_size, num_buckets))),
            socket,
        }
    }

    pub async fn add_node(&self, key: Key, addr: SocketAddr) {
        let rt = self.routing_table.write().await;
        rt.add_node(key, addr).await;
    }

    pub async fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        let rt = self.routing_table.read().await;
        rt.find_node(key).await
    }

    pub async fn join_network_procedure(&self, bootstrap_node: SocketAddr) {

        // 1. Cpy bootstrap nodes' routing table - maybe not necessary as step 2. does this?
        // 2. Query itself against the bootstrap node (FIND_NODE), update RT
        // 3. Iteratively query itself against nodes from 2. and onwards until stable state of routing table (nothing changes anymore)

        //let message = KademliaMessage::new(
        //                 KademliaMessageType::FindNode {
        //                     node_id: self.key.clone(),
        //                 },
        //                 NodeInfo::new(self.key.clone(), None),
        //                 NodeInfo::new(key.clone(), Some(addr.clone())),
        //             );
        //
        //             send_message(message).await;
    }

    ///
    /// This method does listen for incoming messages.
    ///
    pub async fn listen_for_messages(&self) {
        let mut buffer = [0; 1024];

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, src)) => {
                    let message = String::from_utf8_lossy(&buffer[..size]);
                    self.receive_message(message, src);
                }
                Err(e) => {
                    println!("Error while receiving from socket: {}", e);
                }
            }
        }
    }


    ///
    /// This method does determine what to do with received data.
    /// If this receives a request, it will dispatch a new thread to handle it.
    /// If this receives a response, it will pass it to a channel where a process
    /// should be waiting for it.
    ///
    pub fn receive_message(&self, message: Cow<str>, sender: SocketAddr) {
        let routing_table = self.routing_table.clone();
        let parsed_message = kademlia_messages::parse_kademlia_message(message, sender);

        // Handle response
        if parsed_message.get_type().is_response() {
            // todo response
            return;
        }

        // Handle request
        tokio::spawn(async move {
            handle_received_request(parsed_message, routing_table).await;
        });
    }
}
