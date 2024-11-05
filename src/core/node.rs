use super::key::Key;
use super::routing_table::RoutingTable;
use crate::core::kademlia_dht::KademliaDHT;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct Node {
    key: Key,
    address: SocketAddr,
    routing_table: Arc<Mutex<RoutingTable>>,
    dht: Arc<Mutex<KademliaDHT>>,
    socket: UdpSocket,
}

impl Node {
    pub fn new(key: Key, ip: String, port: u16, bucket_size: usize, num_buckets: usize) -> Self {
        let address = format!("{}:{}", ip, port).parse().expect("Invalid address");
        let socket = UdpSocket::bind(address).expect("Failed to bind socket");
        Node {
            key,
            address,
            routing_table: Arc::new(Mutex::new(RoutingTable::new(bucket_size, num_buckets))),
            dht: Arc::new(Mutex::new(KademliaDHT::new(
                Arc::new(Mutex::new(Node::new(
                    key,
                    ip,
                    port,
                    bucket_size,
                    num_buckets,
                ))),
                Arc::new(Mutex::new(RoutingTable::new(bucket_size, num_buckets))),
            ))),
            socket,
        }
    }

    pub fn start(&self) {
        let socket = self.socket.try_clone().expect("Failed to clone socket");
        let routing_table = Arc::clone(&self.routing_table);
        let dht = Arc::clone(&self.dht);
        thread::spawn(move || {
            let mut buf = [0; 1024];
            loop {
                match socket.recv_from(&mut buf) {
                    Ok((size, src)) => {
                        let message = String::from_utf8_lossy(&buf[..size]);
                        println!("Received message from {}: {}", src, message);
                        // Handle the message (placeholder)
                    }
                    Err(e) => {
                        eprintln!("Failed to receive message: {}", e);
                    }
                }
            }
        });
    }

    pub fn add_node(&self, key: Key, addr: SocketAddr) {
        let mut rt = self.routing_table.lock().unwrap();
        rt.add_node(key, addr);
    }

    pub fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        let rt = self.routing_table.lock().unwrap();
        rt.find_node(key)
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
}
