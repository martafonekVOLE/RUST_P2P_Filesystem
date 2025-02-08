use crate::constants::K;
use crate::core::key::Key;
use crate::core::node::{Node, NodeError};
use crate::networking::node_info::NodeInfo;
use std::collections::VecDeque;

use std::time::SystemTime;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum KBucketError {
    #[error("Not enough space in K-bucket")]
    NotEnoughSpace, // TODO: not needed
    #[error("Failed to add nodeinfo")]
    FailedToAdd,
}

#[derive(Clone)]
pub struct KBucket {
    nodes: VecDeque<NodeInfo>,
    capacity: usize,
    pub last_lookup_at: SystemTime,
    /*
    TODO: channel for communication with sender
    */
}

/*
Nodes are kept sorted by last-seen metric
 */

/*
Add node:

    if already exists:
        move to tail (is most rec. seen)
    ->  return

    if enough space in k-bucket:
        Insert
    else:
        Ping node at head (least rec. seen)
        if replies:
            Discard new node (don't insert)
        else:
            Discard the pinged node
            Insert new at tail (is now most rec. seen)
 */

impl KBucket {
    pub fn new() -> Self {
        KBucket {
            nodes: VecDeque::with_capacity(K),
            capacity: K,
            last_lookup_at: SystemTime::now(),
        }
    }

    pub fn set_last_lookup_now(&mut self) {
        self.last_lookup_at = SystemTime::now();
    }

    pub async fn add_nodeinfo(
        &mut self,
        node_info_new: NodeInfo,
        this_node: &Node,
    ) -> Result<(), KBucketError> {
        // Remove if exists
        self.remove_nodeinfo(&node_info_new.id);

        if self.nodes.len() < self.capacity {
            self.nodes.push_back(node_info_new);
            return Ok(());
        }

        let head = self.nodes.remove(0).unwrap();
        match this_node.ping(head.get_id()).await {
            Ok(_) => {
                self.nodes.push_back(head.clone()); // Move to tail (is not MRS)
                Ok(())
            }
            Err(NodeError::ResponseTimeout) | Err(NodeError::BadResponse) => {
                self.nodes.push_back(node_info_new); // Insert new node instead (old didn't reply)
                Ok(())
            }
            // If ping failed for reasons other than no response, return error
            Err(_) => Err(KBucketError::FailedToAdd),
        }

        /*  TODO:
        ping node from head (least rec. seen):
            Send request to channel
            Wait for response from channel

        if replies:
            Discard new node (don't insert)
            -> return false
        else:
            Discard the pinged node
            Insert new at tail (is now most rec. seen)
            -> return true
        */
    }

    pub fn add_nodeinfo_limited(&mut self, node_info: NodeInfo) -> Result<(), KBucketError> {
        // Remove if exists
        self.remove_nodeinfo(&node_info.id);

        if self.nodes.len() < self.capacity {
            self.nodes.push_back(node_info);
            return Ok(());
        }

        Err(KBucketError::NotEnoughSpace)
    }

    pub fn remove_nodeinfo(&mut self, key: &Key) -> Option<NodeInfo> {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == *key) {
            self.nodes.remove(pos)
        } else {
            None
        }
    }

    pub fn get_nodeinfo(&self, key: &Key) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| n.id == *key)
    }

    pub fn get_nodeinfos(&self) -> &VecDeque<NodeInfo> {
        &self.nodes
    }
}

impl Default for KBucket {
    fn default() -> Self {
        KBucket::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::net::{Ipv4Addr, SocketAddr};

    fn create_local_node() -> NodeInfo {
        let id = Key::new_random();
        let port = rand::thread_rng().gen_range(1024..65535);
        let address = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
        NodeInfo { id, address }
    }

    #[test]
    fn test_add_node() {
        let mut kbucket = KBucket::new();
        let node = create_local_node();

        assert!(kbucket.add_nodeinfo_limited(node.clone()).is_ok());
        assert_eq!(kbucket.nodes.len(), 1);
        assert_eq!(kbucket.nodes.back().unwrap(), &node);
    }

    #[test]
    fn test_remove_node() {
        let mut kbucket = KBucket::new();
        let node = create_local_node();

        assert!(kbucket.add_nodeinfo_limited(node.clone()).is_ok());
        assert_eq!(kbucket.nodes.len(), 1);
        assert!(kbucket.remove_nodeinfo(&node.id).is_some());
        assert_eq!(kbucket.nodes.len(), 0);
    }

    #[test]
    fn test_find_node() {
        let mut kbucket = KBucket::new();
        let node = create_local_node();

        assert!(kbucket.add_nodeinfo_limited(node.clone()).is_ok());
        let found_node = kbucket.get_nodeinfo(&node.id);
        assert!(found_node.is_some());
        assert_eq!(found_node.unwrap(), &node);
    }

    #[test]
    fn test_kbucket_capacity() {
        let mut kbucket = KBucket::new();
        for _ in 0..K {
            let node = create_local_node();
            assert!(kbucket.add_nodeinfo_limited(node.clone()).is_ok());
        }
        assert_eq!(kbucket.nodes.len(), K);
    }

    #[test]
    fn test_add_node_existing() {
        let mut kbucket = KBucket::new();
        let node1 = create_local_node();
        let mut node2 = node1.clone();
        node2
            .address
            .set_port(rand::thread_rng().gen_range(1024..65535));

        assert_eq!(&node1.get_id(), &node2.get_id());

        assert!(kbucket.add_nodeinfo_limited(node1.clone()).is_ok());
        assert!(kbucket.add_nodeinfo_limited(node2.clone()).is_ok());
        assert_eq!(kbucket.nodes.len(), 1);
        assert_eq!(kbucket.nodes.back().unwrap(), &node2);
    }

    #[test]
    fn test_add_node_full_capacity() {
        let mut kbucket = KBucket::new();
        for _ in 0..K {
            let node = create_local_node();
            assert!(kbucket.add_nodeinfo_limited(node.clone()).is_ok());
        }
        let new_node = create_local_node();
        assert!(!kbucket.add_nodeinfo_limited(new_node).is_ok());
        assert_eq!(kbucket.nodes.len(), K);
    }
}
