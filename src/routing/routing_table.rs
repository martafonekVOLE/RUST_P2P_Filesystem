use super::kbucket::KBucket;
use crate::core::key::Key;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RoutingTable {
    buckets: Vec<Arc<RwLock<KBucket>>>,
    bucket_size: usize,
}

impl RoutingTable {
    pub fn new(bucket_size: usize, num_buckets: usize) -> Self {
        let mut buckets = Vec::with_capacity(num_buckets);
        for _ in 0..num_buckets {
            buckets.push(Arc::new(RwLock::new(KBucket::new())));
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

    pub async fn add_node(&self, key: Key, addr: SocketAddr) -> bool {
        let index = self.get_bucket_index(&key);
        let mut bucket = self.buckets[index].write().await;
        bucket.add_node(key, addr)
    }

    pub async fn remove_node(&self, key: &Key) -> bool {
        let index = self.get_bucket_index(key);
        let mut bucket = self.buckets[index].write().await;
        bucket.remove_node(key)
    }

    pub async fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        let index = self.get_bucket_index(key);
        let bucket = self.buckets[index].read().await;
        bucket.find_node(key).cloned()
    }

    pub async fn get_all_nodes(&self) -> Vec<(Key, SocketAddr)> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            let bucket = bucket.read().await;
            all_nodes.extend(bucket.get_nodes().iter().cloned());
        }
        all_nodes
    }
}
