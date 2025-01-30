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
use crate::networking::node_info::NodeInfo;
use clap::builder::TypedValueParser;
use clap::Parser;
use cli::args::Arguments;
use config::Config;
use log::{info, warn, LevelFilter};
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
    let config = Config::parse_from_file(&args.config, args.skip_join, args.port)
        .expect("Failed to read configuration file");
    // Get this node's IP
    let ip = config
        .resolve_ip_address()
        .await
        .expect("Failed to resolve IP address");

    // unwrap port in to u16
    let port = config
        .node_port
        .expect("Configuration error: node_port is missing or invalid.");

    // Create node
    let node = Node::new(Key::new_random(), ip.to_string(), port, config.storage_path).await;

    // Begin listening for incoming network traffic
    node.start_listening();

    // Log the node port
    info!("Your node {} is listening at {}", node.key, node.address);

    if args.skip_join {
        warn!("Skipping network join! Only set --skip-join for the first node in a new network.");
    } else {
        // Create the beacon node's info
        let beacon_node_key = config
            .beacon_node_key
            .expect("Configuration error: beacon_node_key is missing");
        let beacon_node_addr = config
            .beacon_node_address
            .expect("Configuration error: beacon_node_address is missing");
        let beacon_node_info = NodeInfo::new(beacon_node_key, beacon_node_addr);

        // Attempt to join the network
        match node.join_network(beacon_node_info).await {
            Ok(_) => {
                info!("Successfully joined the network!");
            }
            Err(e) => {
                warn!("Failed to join the network: {}", e);
                return;
            }
        }
    }

    //
    // Key, metadata (chuck1, chuck2, .. ) checksum
    //
    // file=hash
    //

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
                    match reachable {
                        Ok(reachable) => {
                            println!("Node {} is reachable: {:?}", parts[1], reachable);
                        }
                        Err(e) => {
                            println!("Failed to ping node {}: {}", parts[1], e);
                        }
                    }
                }
                Err(e) => {
                    println!("Invalid node ID: {}", e);
                }
            },
            "find_node" if parts.len() == 2 => match Key::from_hex_str(parts[1]) {
                Ok(node_id) => {
                    let closest_nodes = node.find_node(node_id).await;
                    if let Ok(closest_nodes) = closest_nodes {
                        println!("Closest nodes to {}:", parts[1]);
                        for node in closest_nodes {
                            println!(" - {}", node);
                        }
                    } else {
                        println!("Failed to find closest nodes: {:?}", closest_nodes);
                    }
                }
                Err(e) => {
                    println!("Invalid node ID: {}", e);
                }
            },
            "store" if parts.len() == 3 => {
                let file_path = parts[1];
                match node.store(file_path).await {
                    Ok(()) => {
                        println!("Successfully stored!");
                    }
                    Err(e) => {
                        println!("Failed to store: {}", e);
                    }
                }
            }
            _ => {
                println!("Unknown command or incorrect arguments");
            }
        }
    }
}
