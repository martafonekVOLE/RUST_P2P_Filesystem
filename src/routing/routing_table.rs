use super::kbucket::KBucket;
use crate::config::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RoutingTable {
    id: Key,
    buckets: Vec<Arc<RwLock<KBucket>>>,
    bucket_size: usize,
}

impl RoutingTable {
    // TODO should this be 160?
    pub const NUM_BUCKETS: usize = 160;
    
    pub fn new(id: Key) -> Self {
        let buckets = (0..Self::NUM_BUCKETS)
            .map(|_| Arc::new(RwLock::new(KBucket::new())))
            .collect();
        RoutingTable {
            id,
            buckets,
            bucket_size: K,
        }
    }

    pub async fn store_nodeinfo(&self, node_info: NodeInfo) -> bool {
        let bucket_index = self.id.leading_zeros(&node_info.get_id()) as usize;
        if let Some(bucket) = self.buckets.get(bucket_index) {
            let mut bucket = bucket.write().await;
            bucket.add_node(node_info)
        } else {
            false
        }
    }

    pub fn get_buckets(&self) -> Vec<Arc<RwLock<KBucket>>> {
        self.buckets.clone()
    }
}
