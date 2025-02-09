use super::kbucket::*;
use crate::constants::{ALPHA, K};
use crate::core::key::Key;
use crate::core::node::Node;
use crate::networking::node_info::NodeInfo;

use thiserror::Error;

use std::cmp::Ordering;

#[derive(Error, Debug, PartialEq)]
pub enum RoutingTableError {
    #[error("All k-buckets are empty")]
    Empty,
    #[error("Failed to find k-bucket for ID {id}")]
    BucketNotFoundForId { id: Key },
    #[error("Failed to find k-bucket")]
    BucketNotFound,
    #[error("K-bucket operation failed")]
    KBucketFailed(#[from] KBucketError),
    #[error("Attempting to store self")]
    AttempStoreSelf,
}

const DEFAULT_BUCKET_REFRESH_INTERVAL_S: u64 = 3600; // 1 h

pub struct RoutingTable {
    id: Key,
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    // Must be 160
    pub const NUM_BUCKETS: usize = K * 8; // Key byte length * bits_per_byte

    pub fn new(id: Key) -> Self {
        RoutingTable {
            id,
            buckets: vec![KBucket::new(); Self::NUM_BUCKETS],
        }
    }

    pub async fn store_nodeinfo(
        &mut self,
        node_info: NodeInfo,
        node: &Node,
    ) -> Result<(), RoutingTableError> {
        let bucket_index = self.get_bucket_index(&node_info.id)?;
        if let Some(bucket) = self.buckets.get_mut(bucket_index) {
            bucket.add_nodeinfo(node_info, node).await?;
            Ok(())
        } else {
            Err(RoutingTableError::BucketNotFoundForId { id: node_info.id })
        }
    }

    #[allow(dead_code)] // TODO Remove?
    fn store_nodeinfo_limited(&mut self, node_info: NodeInfo) -> Result<(), RoutingTableError> {
        let bucket_index = self.get_bucket_index(&node_info.id)?;
        if let Some(bucket) = self.buckets.get_mut(bucket_index) {
            bucket.add_nodeinfo_limited(node_info)?;
            Ok(())
        } else {
            Err(RoutingTableError::BucketNotFoundForId { id: node_info.id })
        }
    }

    pub async fn store_nodeinfo_multiple(
        &mut self,
        node_infos: Vec<NodeInfo>,
        node: &Node,
    ) -> Result<(), RoutingTableError> {
        for node_info in node_infos {
            self.store_nodeinfo(node_info, node).await?;
        }
        Ok(())
    }

    pub fn get_nodeinfo(&self, key: &Key) -> Result<Option<&NodeInfo>, RoutingTableError> {
        let bucket_index = self.get_bucket_index(key)?;
        Ok(self.buckets[bucket_index].get_nodeinfo(key))
    }

    /// Do not call this if you initiate the lookup (FIND_NODE/FIND_VALUE), call lookup_... instead
    pub fn get_alpha_closest(&self, key: &Key) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.get_n_closest(key, ALPHA)
    }

    /// Do not call this if you initiate the lookup (FIND_NODE/FIND_VALUE), call lookup_... instead
    pub fn get_k_closest(&self, key: &Key) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.get_n_closest(key, K)
    }

    pub fn lookup_get_alpha_closest(
        &mut self,
        key: &Key,
    ) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.lookup_get_n_closest(key, ALPHA)
    }

    pub fn lookup_get_k_closest(&mut self, key: &Key) -> Result<Vec<NodeInfo>, RoutingTableError> {
        self.lookup_get_n_closest(key, K)
    }

    /// Closest k-bucket has the most leading zeroes in distance
    fn get_bucket_index(&self, key: &Key) -> Result<usize, RoutingTableError> {
        let lz = self.id.leading_zeros_in_distance(key);
        match lz.cmp(&RoutingTable::NUM_BUCKETS) {
            Ordering::Equal => Err(RoutingTableError::AttempStoreSelf),
            Ordering::Greater => Err(RoutingTableError::BucketNotFoundForId { id: *key }),
            Ordering::Less => Ok(RoutingTable::NUM_BUCKETS - 1 - lz),
        }
    }

    fn lookup_get_n_closest(
        &mut self,
        key: &Key,
        n: usize,
    ) -> Result<Vec<NodeInfo>, RoutingTableError> {
        let bucket_index_of_key = self.get_bucket_index(key)?;

        let bucket = &mut self.buckets[bucket_index_of_key];
        // Update last lookup time of k-bucket
        // TODO: maybe find a better way to do this
        bucket.set_last_lookup_now();
        self.get_n_closest(key, n)
    }

    /// Do not call this if you initiate the lookup (FIND_NODE/FIND_VALUE), call lookup_... instead
    #[allow(unused_assignments)]
    fn get_n_closest(&self, key: &Key, n: usize) -> Result<Vec<NodeInfo>, RoutingTableError> {
        /*
        Take all from bucket[i]
        If closest bucket (i) does not contain enough:
            Append all from bucket[i+1]
            Append all from bucket[i-down_i] until take as much as from bucket[i+1]
            sort all by distance, take only n
            // Guarrantees to get closest
            -> return
        else:
            // Faster but not all of them are closest,
            // cause buckets sort nodes based on proximity to current node, not to target node
            -> return
        */

        let mut result = Vec::new();
        let bucket_index_of_key = self.get_bucket_index(key)?;

        let bucket = &self.buckets[bucket_index_of_key];
        result.extend(bucket.get_nodeinfos().iter().cloned());

        let mut taken_some = true;

        let mut i_up = bucket_index_of_key as isize + 1;
        let mut i_down = bucket_index_of_key as isize - 1;
        while result.len() < n && taken_some {
            let mut got_u_len = 0;
            let mut got_d_len = 0;

            taken_some = false;

            // Take one up
            if (i_up as usize) < self.buckets.len() {
                let bucket = &self.buckets[i_up as usize];
                let got_u = bucket.get_nodeinfos().iter().cloned();
                got_u_len = got_u.len();
                result.extend(got_u);

                i_up += 1;
                taken_some = true;
            }

            // Take down as much to match amount taken up
            if i_down >= 0 {
                let bucket = &self.buckets[i_down as usize];
                let got_d = bucket.get_nodeinfos().iter().cloned();
                got_d_len = got_d.len();
                result.extend(got_d);

                i_down -= 1;

                while got_d_len < got_u_len && i_down >= 0 {
                    let bucket = &self.buckets[i_down as usize];
                    let got_d = bucket.get_nodeinfos().iter().cloned();
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

        result.sort_by_key(|node| node.id.distance(key));
        Ok(result.into_iter().take(n).collect())
    }

    // Used for logging
    pub fn get_all_nodeinfos(&self) -> Vec<NodeInfo> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            all_nodes.extend(bucket.get_nodeinfos().iter().cloned());
        }
        all_nodes
    }

    /// Generate random key (id) that is in range of bucket with provided index
    fn get_random_id_for_bucket(&self, bucket_i: usize) -> Key {
        let mut key = Key::new_random();
        // Bucket index to leading zeroes
        let lz = 160 - bucket_i - 1;
        key.make_exactly_n_same_leading_bits_as(&self.id, lz);
        key
    }

    /// Returns random ids for each bucket that was not a target of lookup for constant amount of time
    pub fn get_node_ids_for_refresh(&self) -> Vec<Key> {
        self.buckets
            .iter()
            .enumerate()
            .filter_map(|(index, bucket)| {
                if bucket.last_lookup_at.elapsed().unwrap().as_secs()
                    >= DEFAULT_BUCKET_REFRESH_INTERVAL_S
                {
                    Some(index)
                } else {
                    None
                }
            })
            .map(|index| self.get_random_id_for_bucket(index))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use crate::constants::K;
    use crate::core::key::Key;
    use crate::networking::node_info::NodeInfo;
    use crate::routing::routing_table::*;

    impl NodeInfo {
        pub fn new_local(id: Key) -> Self {
            NodeInfo {
                id,
                address: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080),
            }
        }
    }

    impl RoutingTable {
        fn get_n_closest_slow(
            &self,
            key: &Key,
            n: usize,
        ) -> Result<Vec<NodeInfo>, RoutingTableError> {
            let mut all_nodes: Vec<NodeInfo> = self
                .buckets
                .iter()
                .flat_map(|bucket| bucket.get_nodeinfos().iter().cloned())
                .collect();

            if all_nodes.is_empty() {
                return Err(RoutingTableError::Empty);
            }

            all_nodes.sort_by_key(|node| node.id.distance(key));
            Ok(all_nodes.into_iter().take(n).collect())
        }

        fn fill_with_random_nodes(&mut self, num_nodes: usize) {
            /*
            Will populate only several highest-index buckets,
            since probability of hitting low distance is exponentially low.
            Not good for testing purposes.
            */
            for _ in 0..num_nodes {
                let node = NodeInfo::new_local(Key::new_random());
                self.store_nodeinfo_limited(node).unwrap_or_default();
            }
        }

        fn fill_buckets_random_uniform(&mut self, num_nodes_per_bucket: usize) {
            /*
            for each bucket in reverse:
                lz = increase num leading zeroes
                populate bucket with random keys:
                    gen random key
                    set first lz bits to equal first lz bits of self.id
                    add node to bucket

             */
            for (lz, bucket) in self.buckets.iter_mut().rev().enumerate() {
                for _ in 0..num_nodes_per_bucket {
                    let mut key = Key::new_random();
                    key.make_exactly_n_same_leading_bits_as(&self.id, lz);
                    let lz_true = self.id.leading_zeros_in_distance(&key);
                    assert!(lz_true == lz);

                    bucket
                        .add_nodeinfo_limited(NodeInfo::new_local(key))
                        .unwrap();
                }
            }
        }
    }

    fn get_n_closest_fuzz(n: usize, nodes_per_bucket: usize) -> bool {
        let id = Key::new_random();
        let mut rt = RoutingTable::new(id);

        assert!(nodes_per_bucket <= K);
        rt.fill_buckets_random_uniform(nodes_per_bucket);

        let key_to_find = Key::new_random();

        let got_slow = rt.get_n_closest_slow(&key_to_find, n).unwrap();
        let got_fast = rt.get_n_closest(&key_to_find, n).unwrap();

        let are_same: bool = got_slow.len() == got_fast.len()
            && got_slow.len()
                == got_slow
                    .iter()
                    .zip(&got_fast)
                    .filter(|&(a, b)| a == b)
                    .count();

        are_same
    }

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

        match routing_table.store_nodeinfo_limited(node_info.clone()) {
            Ok(()) => (),
            Err(err) => eprintln!(
                "Failed to store nodeinfo with key: {}. My key: {}. {}",
                node_info.id, id, err
            ),
        }

        // Retrieve the node and verify success
        let result = routing_table.get_nodeinfo(&remote_id).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &node_info);
    }

    #[test]
    fn test_get_nodeinfo_failure() {
        let id = Key::from_input(b"local_node");
        let routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");

        // Attempt to retrieve a non-existent node
        let result = routing_table.get_nodeinfo(&remote_id).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_get_n_closest_slow() {
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

        routing_table.store_nodeinfo_limited(node1.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node2.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node3.clone()).unwrap();

        let num_fetch = 3;

        let closest_nodes = routing_table
            .get_n_closest_slow(&node1.id, num_fetch)
            .unwrap();
        assert_eq!(closest_nodes.len(), num_fetch);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2));
        assert!(closest_nodes.contains(&node3));
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

        routing_table.store_nodeinfo_limited(node1.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node2.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node3.clone()).unwrap();

        let num_fetch = 3;

        let closest_nodes = routing_table.get_n_closest(&node1.id, num_fetch).unwrap();
        assert_eq!(closest_nodes.len(), num_fetch);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2));
        assert!(closest_nodes.contains(&node3));
    }

    #[test]
    fn test_get_n_closest_empty() {
        let id = Key::new_random();
        let mut routing_table = RoutingTable::new(id);

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
            Key::from_bytes(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8080),
        );

        key[0] = 0b00000000;
        key[1] = 0b00000010;
        let node2 = NodeInfo::new(
            Key::from_bytes(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8081),
        );

        key[0] = 0b00000000;
        key[1] = 0b00000011;
        let node3 = NodeInfo::new(
            Key::from_bytes(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8082),
        );

        key[0] = 0b00000001;
        key[1] = 0b00000000;
        let node4 = NodeInfo::new(
            Key::from_bytes(key),
            SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 8083),
        );

        routing_table.store_nodeinfo_limited(node1.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node2.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node3.clone()).unwrap();
        routing_table.store_nodeinfo_limited(node4.clone()).unwrap();

        key[0] = 0b00000000;
        key[1] = 0b00000001;
        let lookup_key = Key::from_bytes(key);
        let num_fetch = 3;
        let closest_nodes = routing_table.get_n_closest(&lookup_key, num_fetch).unwrap();

        // Verify that the closest nodes are indeed the closest
        assert_eq!(closest_nodes.len(), num_fetch);
        assert!(closest_nodes.contains(&node1));
        assert!(closest_nodes.contains(&node2));
        assert!(closest_nodes.contains(&node3));
        // Should not contain node4 cause its much further away from key
    }

    #[test]
    fn test_get_n_closest_fuzz() {
        let num_tries = 1000;
        for _ in 0..num_tries {
            assert!(get_n_closest_fuzz(3, 5));
        }
    }
}
