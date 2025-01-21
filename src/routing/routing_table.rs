use super::kbucket::KBucket;
use crate::config::{ALPHA, K};
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use std::collections::VecDeque;

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RoutingTableError {
    #[error("All k-buckets are empty")]
    Empty,
}

pub struct RoutingTable {
    id: Key,
    buckets: Vec<KBucket>,
    bucket_size: usize, // TODO: remove?
}

impl RoutingTable {
    // Must be 160
    pub const NUM_BUCKETS: usize = K * 8; // Key byte length * bits_per_byte

    pub fn new(id: Key) -> Self {
        RoutingTable {
            id,
            buckets: vec![KBucket::new(); Self::NUM_BUCKETS],
            bucket_size: K,
        }
    }

    // TODO: return Result<()>, propagate error from k-bucket
    pub fn store_nodeinfo(&mut self, node_info: NodeInfo) -> bool {
        let bucket_index = self.id.leading_zeros(&node_info.get_id());
        if let Some(bucket) = self.buckets.get_mut(bucket_index) {
            bucket.add_node(node_info).is_ok() // TODO: propagate with ?
        } else {
            false
        }
    }

    pub fn get_nodeinfo(&self, key: &Key) -> Option<&NodeInfo> {
        let bucket_index = self.id.leading_zeros(key);
        if let Some(bucket) = self.buckets.get(bucket_index) {
            bucket.get_node(key)
        } else {
            None
        }
    }

    // TODO: remove (do not use)
    pub fn get_all_nodeinfos(&self) -> Vec<NodeInfo> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            all_nodes.extend(bucket.get_nodes().iter().cloned());
        }
        all_nodes
    }

    pub fn get_alpha_closest(&self, key: &Key) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.get_n_closest(key, ALPHA)
    }

    pub fn get_k_closest(&self, key: &Key) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.get_n_closest(key, K)
    }

    fn get_n_closest(&self, key: &Key, n: usize) -> Result<Vec<NodeInfo>, RoutingTableError> {
        let mut result = Vec::new();
        let bucket_index = self.id.leading_zeros(key);

        // Check the initial bucket
        if let Some(bucket) = self.buckets.get(bucket_index) {
            result.extend(bucket.get_nodes().iter().cloned());
            if result.len() >= n {
                result.sort_by_key(|node| node.get_id().distance(key));
                return Ok(result.into_iter().take(n).collect());
            }
        }

        // Check lower buckets
        for i in (0..bucket_index).rev() {
            if let Some(bucket) = self.buckets.get(i) {
                result.extend(bucket.get_nodes().iter().cloned());
                if result.len() >= n {
                    result.sort_by_key(|node| node.get_id().distance(key));
                    return Ok(result.into_iter().take(n).collect());
                }
            }
        }

        // Check higher buckets
        for i in (bucket_index + 1)..self.buckets.len() {
            if let Some(bucket) = self.buckets.get(i) {
                result.extend(bucket.get_nodes().iter().cloned());
                if result.len() >= n {
                    result.sort_by_key(|node| node.get_id().distance(key));
                    return Ok(result.into_iter().take(n).collect());
                }
            }
        }

        if result.is_empty() {
            return Err(RoutingTableError::Empty);
        }

        // Sort and return all found nodes, less than N
        result.sort_by_key(|node| node.get_id().distance(key));
        Ok(result.into_iter().take(n).collect())
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::core::key::Key;
    use crate::networking::node_info::NodeInfo;
    use crate::routing::routing_table::*;

    #[test]
    fn test_initialize() {
        let id = Key::new_random();
        let routing_table = RoutingTable::new(id);

        // Verify the number of buckets
        assert_eq!(routing_table.buckets.len(), RoutingTable::NUM_BUCKETS);
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

        assert!(bucket.get_node(&node_info.get_id()).is_some());
    }

    #[test]
    fn test_get_nodeinfo_success() {
        let id = Key::from_input(b"local_node");
        let mut routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");
        let remote_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let node_info = NodeInfo::new(remote_id, remote_address);

        // Store the node
        routing_table.store_nodeinfo(node_info.clone());

        // Retrieve the node and verify success
        let result = routing_table.get_nodeinfo(&remote_id);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &node_info);
    }

    #[test]
    fn test_get_nodeinfo_failure() {
        let id = Key::from_input(b"local_node");
        let routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");

        // Attempt to retrieve a non-existent node
        let result = routing_table.get_nodeinfo(&remote_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_n_closest_success() {
        let id = Key::new_random();
        let mut routing_table = RoutingTable::new(id);

        let node1 = NodeInfo::new(
            Key::new_random(),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080),
        );
        let node2 = NodeInfo::new(
            Key::new_random(),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8081),
        );
        let node3 = NodeInfo::new(
            Key::new_random(),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8082),
        );

        routing_table.store_nodeinfo(node1.clone());
        routing_table.store_nodeinfo(node2.clone());
        routing_table.store_nodeinfo(node3.clone());

        let closest_nodes = routing_table.get_n_closest(&node1.get_id(), 2).unwrap();
        assert_eq!(closest_nodes.len(), 2);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2) || closest_nodes.contains(&node3));
    }

    #[test]
    fn test_get_n_closest_empty() {
        let id = Key::new_random();
        let routing_table = RoutingTable::new(id);

        let node_id = Key::new_random();
        let result = routing_table.get_n_closest(&node_id, 2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), RoutingTableError::Empty);
    }
}
