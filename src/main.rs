mod cli;
mod config;
mod constants;
mod core;
mod networking;
mod routing;
mod sharding;
mod storage;
mod utils;

use crate::core::key::Key;
use crate::core::node::Node;
use clap::Parser;
use cli::args::Arguments;
use config::Config;
use log::LevelFilter;
use p2p::utils::logging::init_logging;
use public_ip;
use std::io::BufRead;

#[tokio::main]
async fn main() {
    // Parse command-line arguments
    let args = Arguments::parse();

    // Initialize logging
    if args.verbose {
        // init_logging(LevelFilter::Info)
    } else {
        // init_logging(LevelFilter::Warn)
    }
    // For debug purposes
    init_logging(LevelFilter::Debug);

    // Read and parse the configuration file
    let config = Config::read_from_file(&args.config).expect("Failed to read configuration file");
    let ip = config
        .resolve_ip_address()
        .await
        .expect("Failed to resolve IP address");

    println!("Arguments: {:?}", args);
    println!("Configuration: {:?}", config);
    println!("Resolved IP address: {}", ip);

    // Create node
    let node = Node::new(Key::new_random(), ip.to_string(), config.node_port).await;

    // Begin listening for incoming network traffic
    node.start_listening();

    // Synchronous loop reading stdin lines; pass commands to node API
    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.expect("Failed to read line from stdin");
        let parts: Vec<&str> = line.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "ping" if parts.len() == 2 => match Key::from_hex_str(parts[1]) {
                Ok(node_id) => {
                    let reachable = node.ping(node_id).await;
                    println!("Node {} is reachable: {}", parts[1], reachable);
                }
                Err(e) => {
                    println!("Invalid node ID: {}", e);
                }
            },
            "find_node" if parts.len() == 2 => match Key::from_hex_str(parts[1]) {
                Ok(node_id) => {
                    let closest_nodes = node.find_node(node_id).await;
                    println!("Closest nodes to {}: {:?}", parts[1], closest_nodes);
                }
                Err(e) => {
                    println!("Invalid node ID: {}", e);
                }
            },
            _ => {
                println!("Unknown command or incorrect arguments");
            }
        }
    }
}
