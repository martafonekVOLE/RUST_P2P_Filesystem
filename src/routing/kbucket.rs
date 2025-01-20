use crate::constants::K;
use crate::core::key::Key;
use std::collections::VecDeque;

use crate::networking::node_info::NodeInfo;

#[derive(Clone)]
pub struct KBucket {
    nodes: VecDeque<NodeInfo>,
    capacity: usize,
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
        }
    }

    pub fn add_node(&mut self, node: NodeInfo) -> bool {
        // Remove if exists
        self.remove_node(&node.id);

        if self.nodes.len() < self.capacity {
            self.nodes.push_back(node);
            return true;
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

        false
    }

    pub fn remove_node(&mut self, key: &Key) -> bool {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == *key) {
            self.nodes.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn find_node(&self, key: &Key) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| n.id == *key)
    }

    pub fn get_nodes(&self) -> &VecDeque<NodeInfo> {
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

        assert!(kbucket.add_node(node.clone()));
        assert_eq!(kbucket.nodes.len(), 1);
        assert_eq!(kbucket.nodes.back().unwrap(), &node);
    }

    #[test]
    fn test_remove_node() {
        let mut kbucket = KBucket::new();
        let node = create_local_node();

        assert!(kbucket.add_node(node.clone()));
        assert_eq!(kbucket.nodes.len(), 1);
        assert!(kbucket.remove_node(&node.id));
        assert_eq!(kbucket.nodes.len(), 0);
    }

    #[test]
    fn test_find_node() {
        let mut kbucket = KBucket::new();
        let node = create_local_node();

        assert!(kbucket.add_node(node.clone()));
        let found_node = kbucket.find_node(&node.id);
        assert!(found_node.is_some());
        assert_eq!(found_node.unwrap(), &node);
    }

    #[test]
    fn test_kbucket_capacity() {
        let mut kbucket = KBucket::new();
        for _ in 0..K {
            let node = create_local_node();
            assert!(kbucket.add_node(node.clone()));
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

        assert!(kbucket.add_node(node1.clone()));
        assert!(kbucket.add_node(node2.clone()));
        assert_eq!(kbucket.nodes.len(), 1);
        assert_eq!(kbucket.nodes.back().unwrap(), &node2);
    }

    #[test]
    fn test_add_node_full_capacity() {
        let mut kbucket = KBucket::new();
        for _ in 0..K {
            let node = create_local_node();
            assert!(kbucket.add_node(node.clone()));
        }
        let new_node = create_local_node();
        assert!(!kbucket.add_node(new_node));
        assert_eq!(kbucket.nodes.len(), K);
    }
}
