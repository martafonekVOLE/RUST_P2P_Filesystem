// src/dht/kbucket.rs

use super::key::Key;
use std::collections::VecDeque;
use std::net::SocketAddr;

#[derive(Clone)]
pub struct KBucket {
    nodes: VecDeque<(Key, SocketAddr)>,
    capacity: usize,
}

impl KBucket {
    pub fn new(capacity: usize) -> Self {
        KBucket {
            nodes: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn add_node(&mut self, key: Key, addr: SocketAddr) -> bool {
        if self.nodes.len() < self.capacity {
            self.nodes.push_back((key, addr));
            true
        } else {
            false
        }
    }

    pub fn remove_node(&mut self, key: &Key) -> bool {
        if let Some(pos) = self.nodes.iter().position(|(k, _)| k == key) {
            self.nodes.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn find_node(&self, key: &Key) -> Option<&(Key, SocketAddr)> {
        self.nodes.iter().find(|(k, _)| k == key)
    }

    pub fn get_nodes(&self) -> &VecDeque<(Key, SocketAddr)> {
        &self.nodes
    }
}