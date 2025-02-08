mod cli;

use clap::builder::TypedValueParser;
use clap::Parser;
use cli::args::Arguments;
use log::{error, info, warn, LevelFilter};
use p2p::config::Config;
use p2p::constants::K;
use p2p::core::key::Key;
use p2p::core::node::Node;
use p2p::networking::node_info::NodeInfo;
use p2p::sharding::common::FileMetadata;
use p2p::utils::logging::init_logging;
use std::io::BufRead;
use std::path::Path;
use std::str::FromStr;

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
    let node = match Node::new(Key::new_random(), ip.to_string(), port, config.storage_path).await {
        Ok(node) => node,
        Err(e) => {
            error!("Failed to create node: {}", e);
            return;
        }
    };

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
            Ok(_) => {}
            Err(e) => {
                error!("Failed to join the network: {}", e);
                return;
            }
        }
    }

    println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    println!(
        "Welcome to the network! Your node is {}",
        node.to_node_info()
    );
    println!("Available commands:");
    println!(" - ping <key>: Send a PING request to the specified node");
    println!(" - find_node <key>: Resolves {} closest nodes to <key>", K);
    println!(" - store <filepath>: Upload a file to the network");
    println!(" - find_value <file_handle> <storage_dir>: Download a file from the network");
    println!(" - dump_rt: Display the contents of the routing table");
    println!(
        "Note: <key> should be a {}-character hexadecimal string",
        K * 2
    );
    println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
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
                    let response = node.ping(node_id).await;
                    match response {
                        Ok(response) => {
                            println!("{} responded to PING: {}", parts[1], response);
                        }
                        Err(e) => {
                            eprintln!("{} failed to respond to PING: {}", parts[1], e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Invalid node ID: {}", e);
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
                        eprintln!("Failed to find closest nodes: {:?}", closest_nodes);
                    }
                }
                Err(e) => {
                    eprintln!("Invalid node ID: {}", e);
                }
            },
            "store" if parts.len() == 2 => {
                let file_path = parts[1];
                let file_metadata = node.upload_file(file_path).await;
                match file_metadata {
                    Ok(file_metadata) => {
                        println!("File uploaded successfully!");
                        println!("File handle: {}", file_metadata);
                    }
                    Err(e) => {
                        eprintln!("Failed to upload file: {}", e);
                    }
                }
            }
            "find_value" if parts.len() == 3 => {
                let file_handle = parts[1];
                let storage_dir = Path::new(parts[2]);
                match FileMetadata::from_str(file_handle) {
                    Ok(file_metadata) => {
                        let file = node.download_file(file_metadata, storage_dir).await;
                        match file {
                            Ok(file) => {
                                println!("File downloaded successfully!");
                                println!("File saved to: {:?}", file);
                            }
                            Err(e) => {
                                eprintln!("Failed to download file: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Invalid file handle: {}", e);
                    }
                }
            }
            "dump_rt" if parts.len() == 1 => {
                let all_contacts = node.get_routing_table_content().await;
                println!("Routing table content:");
                for contact in all_contacts {
                    println!(" - {}", contact);
                }
            }
            "dump_chunks" if parts.len() == 1 => {
                let all_shards = node.get_owned_chunk_keys().await;
                println!("Available chunks:");
                for shard in all_shards {
                    println!(" - {}", shard);
                }
            }
            _ => {
                eprintln!(
                    "Wrong command or syntax '{}', should be 'dump_rt', 'find_node <key>', 'ping <key>', 'store <filepath>' or 'find_value <file_handle> <storage_dir>'",
                    parts[0]
                );
            }
        }
        println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    }
}
