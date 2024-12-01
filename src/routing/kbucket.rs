use crate::config::K;
use crate::core::key::Key;
use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::net::SocketAddr;

/*
It is known that nodes which have been connected for a long time
in a network will probably remain connected for a long time in the future.
Due to this statistical distribution,
Kademlia selects long connected nodes to remain stored in the k-buckets.
This increases the number of known valid nodes at some time in the future and provides
for a more stable network.

When a k-bucket is full and a new node is discovered for that k-bucket,
the least recently seen node in the k-bucket is PINGed.
If the node is found to be still alive,
the new node is placed in a secondary list, a replacement cache.
The replacement cache is used only if a node in the k-bucket stops responding.
In other words: new nodes are used only when older nodes disappear.
*/

/*
Need to also store some additional info about nodes in K-bucket.
For example last time they were seen (received response from PING).
Based on this decide which node to remove from k-bucket in case there is no space left.
*/

#[derive(Clone)]
pub struct NodeRecord {
    key: Key,
    addr_port: SocketAddr,
    last_seen_ms: u64,
}

impl NodeRecord {
    fn new(key: Key, addr_port: SocketAddr) -> Self {
        NodeRecord {
            key,
            addr_port,
            last_seen_ms: 0,
        }
    }
}

trait RemoveNodeLastSeen {
    fn get_nodes_mut(&mut self) -> &mut VecDeque<NodeRecord>;

    fn remove_node_based_on_metric(&mut self) -> Result<NodeRecord> {
        let nodes = self.get_nodes_mut();

        let (index, _) = nodes
            .iter()
            .enumerate()
            .max_by_key(|&(_, n)| n.last_seen_ms)
            .context("Failed to find least recently seen node.")?;

        Ok(nodes.remove(index).context("")?)
    }
}

#[derive(Clone)]
pub struct KBucket {
    nodes: VecDeque<NodeRecord>,
    capacity: usize,
}

impl KBucket {
    pub fn new() -> Self {
        KBucket {
            nodes: VecDeque::with_capacity(K),
            capacity: K,
        }
    }

    pub fn add_node(&mut self, key: Key, addr: SocketAddr) -> bool {
        if self.nodes.len() >= self.capacity && self.remove_node_based_on_metric().is_err() {
            return false;
        }

        self.nodes.push_back(NodeRecord::new(key, addr));
        true
    }

    pub fn remove_node(&mut self, key: &Key) -> bool {
        if let Some(pos) = self.nodes.iter().position(|n| n.key == *key) {
            self.nodes.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn find_node(&self, key: &Key) -> Option<(Key, SocketAddr)> {
        self.nodes
            .iter()
            .map(|n| (n.key, n.addr_port))
            .find(|(k, _)| k == key)
    }

    pub fn get_nodes(&self) -> Vec<(Key, SocketAddr)> {
        self.nodes.iter().map(|n| (n.key, n.addr_port)).collect()
    }
}

impl RemoveNodeLastSeen for KBucket {
    fn get_nodes_mut(&mut self) -> &mut VecDeque<NodeRecord> {
        &mut self.nodes
    }
}
