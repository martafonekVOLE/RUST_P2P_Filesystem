use super::kbucket::KBucket;
use crate::config::K;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct RoutingTable {
    id: Key,
    //buckets: Vec<Arc<RwLock<KBucket>>>,
    buckets: Vec<KBucket>,
    bucket_size: usize, // TODO: remove?
}

impl RoutingTable {
    // Must be 160
    pub const NUM_BUCKETS: usize = K * 8; // Key byte length * bits_per_byte

    pub fn new(id: Key) -> Self {
        // let buckets = (0..Self::NUM_BUCKETS)
        //     .map(|_| Arc::new(RwLock::new(KBucket::new())))
        //     .collect();
        RoutingTable {
            id,
            buckets: vec![KBucket::new(); Self::NUM_BUCKETS],
            bucket_size: K,
        }
    }

    pub fn store_nodeinfo(&mut self, node_info: NodeInfo) -> bool {
        let bucket_index = self.id.leading_zeros(&node_info.get_id());
        if let Some(bucket) = self.buckets.get_mut(bucket_index) {
            bucket.add_node(node_info)
        } else {
            false
        }
    }

    // pub fn get_buckets(&self) -> Vec<Arc<RwLock<KBucket>>> {
    //     self.buckets.clone()
    // }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::core::key::Key;
    use crate::networking::node_info::NodeInfo;
    use crate::routing::routing_table::RoutingTable;

    #[test]
    fn test_initialize() {
        let id = Key::new_random();
        let routing_table = RoutingTable::new(id);

        // Verify the number of buckets
        assert_eq!(routing_table.buckets.len(), RoutingTable::NUM_BUCKETS);
        // for bucket in &routing_table.get_buckets() {
        //     assert!(bucket.read()); // Ensure each bucket is initialized
        // }
    }
    #[test]
    fn test_store_nodeinfo_success() {
        let id = Key::from_input(b"local_node");
        let mut routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");
        let remote_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let node_info = NodeInfo::new(remote_id, remote_address);

        // Store the node and verify success
        let result = routing_table.store_nodeinfo(node_info.clone());
        assert!(result);

        // Verify the node is added to the correct bucket
        let bucket_index = id.leading_zeros(&remote_id);
        let bucket = routing_table.buckets.get(bucket_index).unwrap();
        //let bucket = bucket.read().await;

        assert!(bucket.find_node(&node_info.get_id()).is_some());
    }
}
