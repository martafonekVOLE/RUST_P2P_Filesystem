use crate::core::key::Key;
use crate::core::node::Node;

mod cli;
mod config;
mod core;
mod networking;
mod routing;
mod sharding;
mod storage;
mod utils;

fn main() {
    // Create new node (self) and spawn a thread to handle incoming requests
    let node = Node::new(Key::new_random(), String::from("12.12.12.12"), 65530u16, 12, 1);
    tokio::spawn(async move {
        node.listen_for_messages().await;
    });
}
