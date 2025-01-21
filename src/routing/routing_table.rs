use super::kbucket::*;
use crate::config::{ALPHA, K};
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RoutingTableError {
    #[error("All k-buckets are empty")]
    Empty,
    #[error("Failed to find k-bucket for ID {id}")]
    BucketNotFoundForId { id: Key },
    #[error("K-bucket operation failed")]
    KBucketFailedError(#[from] KBucketError),
}

impl From<RoutingTableError> for String {
    fn from(error: RoutingTableError) -> Self {
        error.to_string()
    }
}

pub struct RoutingTable {
    id: Key,
    buckets: Vec<KBucket>,
    bucket_size: usize, // TODO: remove?
}

impl RoutingTable {
    // Must be 160
    pub const NUM_BUCKETS: usize = K * 8; // Key byte length * bits_per_byte

    fn init(&mut self) {}

    pub fn new(id: Key) -> Self {
        RoutingTable {
            id,
            buckets: vec![KBucket::new(); Self::NUM_BUCKETS],
            bucket_size: K,
        }
    }

    // TODO: return Result<()>, propagate error from k-bucket
    pub fn store_nodeinfo(&mut self, node_info: NodeInfo) -> Result<(), RoutingTableError> {
        let bucket_index = self.get_bucket_index(&node_info.get_id());
        if let Some(bucket) = self.buckets.get_mut(bucket_index) {
            bucket.add_node(node_info)?;
            Ok(())
        } else {
            Err(RoutingTableError::BucketNotFoundForId {
                id: node_info.get_id(),
            })
        }
    }

    pub fn get_nodeinfo(&self, key: &Key) -> Option<&NodeInfo> {
        let bucket_index = self.get_bucket_index(key);
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

    fn get_bucket_index(&self, key: &Key) -> usize {
        RoutingTable::NUM_BUCKETS - 1 - self.id.leading_zeros(key)
    }

    fn get_n_closest(&self, key: &Key, n: usize) -> Result<Vec<NodeInfo>, RoutingTableError> {
        let mut result = Vec::new();
        let bucket_index_of_key = self.get_bucket_index(key);

        if let Some(bucket) = self.buckets.get(bucket_index_of_key) {
            result.extend(bucket.get_nodes().iter().cloned());
        }

        /*
        Take up if not hit ceil -> got U
        Take down if not hit ceil and while not got D >= U
        if D + U >= A
            enough
        else
            repeat
        */

        let mut taken_some = true;

        let mut i_up = bucket_index_of_key as isize + 1;
        let mut i_down = bucket_index_of_key as isize - 1;
        while result.len() < n && taken_some {
            let mut got_u_len = 0;
            let mut got_d_len = 0;

            taken_some = false;

            // Take one up
            if i_up >= 0 && (i_up as usize) < self.buckets.len() {
                let bucket = &self.buckets[i_up as usize];
                let got_u = bucket.get_nodes().iter().cloned();
                got_u_len = got_u.len();
                result.extend(got_u);

                i_up += 1;
                taken_some = true;
            }

            // Take down as much to match amount taken up
            if i_down >= 0 && (i_down as usize) < self.buckets.len() {
                let bucket = &self.buckets[i_down as usize];
                let got_d = bucket.get_nodes().iter().cloned();
                got_d_len = got_d.len();
                result.extend(got_d);

                i_down -= 1;

                while got_d_len < got_u_len && i_down >= 0 {
                    let bucket = &self.buckets[i_down as usize];
                    let got_d = bucket.get_nodes().iter().cloned();
                    got_d_len += got_d.len();
                    result.extend(got_d);

                    i_down -= 1;
                }

                taken_some = true;
            }
        }

        if result.is_empty() {
            return Err(RoutingTableError::Empty);
        }

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
    fn test_get_nodeinfo_success() {
        let id = Key::from_input(b"local_node");
        let mut routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");
        let remote_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let node_info = NodeInfo::new(remote_id, remote_address);

        // Store the node
        assert!(routing_table.store_nodeinfo(node_info.clone()).is_ok());

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

        routing_table.store_nodeinfo(node1.clone()).unwrap();
        routing_table.store_nodeinfo(node2.clone()).unwrap();
        routing_table.store_nodeinfo(node3.clone()).unwrap();

        let num_fetch = 3;

        let closest_nodes = routing_table
            .get_n_closest(&node1.get_id(), num_fetch)
            .unwrap();
        assert_eq!(closest_nodes.len(), num_fetch);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2));
        assert!(closest_nodes.contains(&node3));
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

    #[test]
    fn test_get_n_closest_advanced() {
        let id = Key::new_random();
        let mut routing_table = RoutingTable::new(id);

        // Create nodes with keys having a certain number of leading zeroes
        let mut key = [0u8; 20];

        key[0] = 0b00000000;
        key[1] = 0b00000001;
        let node1 = NodeInfo::new(
            Key::new_raw(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080),
        );

        key[1] = 0b00000010;
        let node2 = NodeInfo::new(
            Key::new_raw(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8081),
        );

        key[1] = 0b00000011;
        let node3 = NodeInfo::new(
            Key::new_raw(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8082),
        );

        key[0] = 0b00000001;
        key[1] = 0b00000000;
        let node4 = NodeInfo::new(
            Key::new_raw(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8083),
        );

        routing_table.store_nodeinfo(node1.clone()).unwrap();
        routing_table.store_nodeinfo(node2.clone()).unwrap();
        routing_table.store_nodeinfo(node3.clone()).unwrap();
        routing_table.store_nodeinfo(node4.clone()).unwrap();

        key[0] = 0b00000000;
        key[1] = 0b00000001;
        //key[2] = 0b10000000;
        let lookup_key = Key::new_raw(key);
        let num_fetch = 3;
        let closest_nodes = routing_table.get_n_closest(&lookup_key, num_fetch).unwrap();

        // Verify that the closest nodes are indeed the closest
        assert_eq!(closest_nodes.len(), num_fetch);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2));
        assert!(closest_nodes.contains(&node3));
    }
}
