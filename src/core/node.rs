use crate::core::key::Key;
use crate::routing::kademlia_messages;
use crate::routing::kademlia_messages::KademliaMessageType;
use crate::routing::routing_table::RoutingTable;
use sha1::Digest;
use std::borrow::Cow;
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct Node {
    key: Key,
    address: SocketAddr,
    socket: UdpSocket,
    routing_table: Arc<RwLock<RoutingTable>>,
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

    pub fn send_message(&self, target: &SocketAddr, message: &str) {
        self.socket
            .send_to(message.as_bytes(), target)
            .expect("Failed to send message");
    }

    pub fn receive_message(&self, message: Cow<str>) {
        let _parsed_message = kademlia_messages::parse_kademlia_message(message);
    }

    pub async fn join_network_procedure(&self, bootstrap_nodes: Vec<(Key, SocketAddr)>) {
        for (key, addr) in bootstrap_nodes {
            let message = kademlia_messages::build_kademlia_message(
                KademliaMessageType::FindNode,
                self.key.clone().to_string(),
                key.to_string(),
            );

            self.send_message(&addr, &message);
        }
    }

    pub async fn listen_for_messages(&self) {
        let mut buffer = [0; 1024];

        loop {
            match self.socket.recv_from(&mut buffer) {
                Ok((size, _src)) => {
                    let message = String::from_utf8_lossy(&buffer[..size]);
                    self.receive_message(message);
                }
                Err(e) => {
                    println!("Error while receiving from socket: {}", e);
                }
            }
        }
    }
}
