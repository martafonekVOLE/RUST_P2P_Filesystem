mod cli;

use clap::builder::TypedValueParser;
use clap::Parser;
use cli::args::Arguments;
use log::{error, info, warn, LevelFilter};
use p2p::cache::Cache;
use p2p::config::Config;
use p2p::constants::K;
use p2p::core::key::Key;
use p2p::core::node::Node;
use p2p::networking::node_info::NodeInfo;
use p2p::utils::logging::init_logging;
use public_ip;
use tokio::time::sleep;
use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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

    // Read and parse the configuration & cache file
    let mut cache = if let Some(cache_path) = args.cache {
        // Load the cache from the cache file.
        Cache::parse_from_file(&cache_path).expect("Failed to read cache file")
    } else if let Some(config_path) = args.config {
        // Load the configuration directly from the config file.
        let config = Config::parse_from_file(&config_path, args.skip_join, args.port)
            .expect("Failed to read configuration file");
        // Construct a Cache object from the loaded configuration.
        Cache { key: Key::new_random(), routing_table_nodes: None, skip_join: args.skip_join, config }
    } else {
        unreachable!("Either --cache or --config must be provided");
    };

    let config = cache.config.clone();

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
    let mut node = Node::new(cache.key, ip.to_string(), port, config.storage_path).await;

    if let Some(routing_nodes) = &cache.routing_table_nodes {
        let mut rt = node.routing_table.write().await;
        for node_info in routing_nodes {
            let _ = rt.store_nodeinfo(node_info.clone());
        }
        info!("Loaded {} nodes into the routing table from cache.", routing_nodes.len());
    }

    // Begin listening for incoming network traffic
    node.start_listening();

    // Log the node port
    info!("Your node {} is listening at {}", node.key, node.address);

    if cache.skip_join {
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
                warn!("Failed to join the network: {}", e);
                return;
            }
        }
    }

    let cache_ref = Arc::new(Mutex::new(cache));
    let node_ref = Arc::new(node.clone());
    
    // Create cache file
    if let Some(file_path) = config.cache_file_path.clone() {
        let node_for_saver = Arc::clone(&node_ref);
        let cache_for_saver = Arc::clone(&cache_ref);

        tokio::spawn(async move {
            let interval = Duration::from_secs(5);
            loop {
                // Fetch routing table from the node
                let all_contacts = node_for_saver.get_routing_table_content().await;

                // Lock the cache, update, then save
                {
                    // Lock the cache and unwrap the guard
                    let mut locked_cache = cache_for_saver.lock().unwrap();
                    
                    // Now you can access the fields inside `Cache`
                    locked_cache.routing_table_nodes = Some(all_contacts);
                
                    // And call any methods as well
                    if let Err(e) = locked_cache.save_to_file(&file_path) {
                        eprintln!("Failed to save cache file: {}", e);
                    }
                }

                // Sleep for 5 seconds
                sleep(interval).await;
            }
        });
    }

    println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    println!(
        "Welcome to the network! Your node is {}",
        node.to_node_info()
    );
    println!("Available commands:");
    println!(" - ping <key>: Send a PING request to the specified node");
    println!(" - find_node <key>: Resolves {} closest nodes to <key>", K);
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
                match node.store(file_path).await {
                    Ok(()) => {
                        println!("Successfully stored!");
                    }
                    Err(e) => {
                        println!("Failed to store: {}", e);
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
            _ => {
                eprintln!(
                    "Unknown command '{}', should be 'dump_rt', 'find_node <key>', 'ping <key>'",
                    parts[0]
                );
            }
        }
        println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    }
}
