use crate::config::K;
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
        if self.remove_node(&node.id) {
            self.nodes.push_back(node);
            return true;
        }

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
        else:
            Discard the pinged node
            Insert new at tail (is now most rec. seen)
        */

        return false;
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
