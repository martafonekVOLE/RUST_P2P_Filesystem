use crate::core::key::Key;
use crate::routing::routing_table::RoutingTable;
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
        let mut rt = self.routing_table.write().await;
        rt.add_node(key, addr).await;
    }

    pub async fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        let mut rt = self.routing_table.read().await;
        rt.find_node(key).await
    }

    pub fn send_message(&self, target: &SocketAddr, message: &str) {
        self.socket
            .send_to(message.as_bytes(), target)
            .expect("Failed to send message");
    }

    pub fn receive_message(&self, message: &str) {
        println!("Received message: {}", message);
        // Handle the message (placeholder)
    }

    pub fn join_network_procedure() {
        // Placeholder for the join network procedure
    }
}
