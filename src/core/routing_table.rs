// src/dht/routing_table.rs

use super::key::Key;
use super::kbucket::KBucket;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub struct RoutingTable {
    buckets: Vec<Arc<Mutex<KBucket>>>,
    bucket_size: usize,
}

impl RoutingTable {
    pub fn new(bucket_size: usize, num_buckets: usize) -> Self {
        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(Arc::new(Mutex::new(KBucket::new(bucket_size))));
        }
        RoutingTable {
            buckets,
            bucket_size,
        }
    }

    fn get_bucket_index(&self, key: &Key) -> usize {
        // Simplified bucket index calculation
        key.value[0] as usize % self.buckets.len()
    }

    pub fn add_node(&self, key: Key, addr: SocketAddr) -> bool {
        let index = self.get_bucket_index(&key);
        let mut bucket = self.buckets[index].lock().unwrap();
        bucket.add_node(key, addr)
    }

    pub fn remove_node(&self, key: &Key) -> bool {
        let index = self.get_bucket_index(key);
        let mut bucket = self.buckets[index].lock().unwrap();
        bucket.remove_node(key)
    }

    pub fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        let index = self.get_bucket_index(key);
        let bucket = self.buckets[index].lock().unwrap();
        bucket.find_node(key).cloned()
    }

    pub fn get_all_nodes(&self) -> Vec<(Key, SocketAddr)> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            let bucket = bucket.lock().unwrap();
            all_nodes.extend(bucket.get_nodes().iter().cloned());
        }
        all_nodes
    }
}