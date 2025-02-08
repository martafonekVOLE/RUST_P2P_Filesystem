mod cli;

use clap::Parser;
use cli::args::Arguments;
use log::{error, info, warn, LevelFilter};
use p2p::cache::Cache;
use p2p::config::Config;
use p2p::constants::K;
use p2p::core::key::Key;
use p2p::core::node::Node;
use p2p::networking::node_info::NodeInfo;
use p2p::sharding::common::FileMetadata;
use p2p::utils::logging::init_logging;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command-line arguments
    let args = Arguments::parse();

    // Initialize logging based on verbosity
    if args.verbose {
        init_logging(LevelFilter::Info);
    } else {
        init_logging(LevelFilter::Warn);
    }
    // Load the configuration & cache file
    let cache = if let Some(cache_path) = args.cache {
        Cache::parse_from_file(&cache_path).map_err(|e| {
            error!("Failed to read cache file '{}': {}", cache_path, e);
            e
        })?
    } else if let Some(config_path) = args.config {
        let config =
            Config::parse_from_file(&config_path, args.skip_join, args.port).map_err(|e| {
                error!("Failed to read configuration file '{}': {}", config_path, e);
                e
            })?;
        Cache {
            key: Key::new_random(),
            routing_table_nodes: None,
            skip_join: args.skip_join,
            config,
        }
    } else {
        error!("Either --cache or --config must be provided");
        return Err("Either --cache or --config must be provided".into());
    };

    let mut config = cache.config.clone();

    // Get this node's IP address
    let ip = config.resolve_ip_address().await.map_err(|e| {
        error!("Failed to resolve IP address: {}", e);
        e
    })?;

    // Get the node port (u16)
    let port = config.node_port.ok_or_else(|| {
        error!("Configuration error: node_port is missing or invalid.");
        "Configuration error: node_port is missing or invalid."
    })?;

    // Create a new random key for this node
    let node_key = Key::new_random();

    // Create automatic storage directory if specified
    if args.automatic_storage {
        let storage_path = format!("storage-{}", node_key);
        if let Err(e) = std::fs::create_dir_all(&storage_path) {
            error!(
                "Failed to create storage directory '{}': {}",
                storage_path, e
            );
            return Err(e.into());
        }
        config.storage_path = storage_path;
    }

    // Create the node
    let node = Node::new(node_key, ip.to_string(), port, config.storage_path.clone())
        .await
        .map_err(|e| {
            error!("Failed to create node: {}", e);
            e
        })?;
    let node = Arc::new(node);

    // Begin listening for incoming network traffic
    node.clone().start_listening();

    info!("Your node {} is listening at {}", node.key, node.address);

    // Join the network if required
    if cache.skip_join {
        warn!("Skipping network join! Only set --skip-join for the first node in a new network.");
    } else {
        // Get beacon node info from config
        let beacon_node_key = config.beacon_node_key.ok_or_else(|| {
            error!("Configuration error: beacon_node_key is missing");
            "Configuration error: beacon_node_key is missing"
        })?;
        let beacon_node_addr = config.beacon_node_address.ok_or_else(|| {
            error!("Configuration error: beacon_node_address is missing");
            "Configuration error: beacon_node_address is missing"
        })?;
        let beacon_node_info = NodeInfo::new(beacon_node_key, beacon_node_addr);

        // Attempt to join the network
        println!("Connecting to the network...");
        node.join_network(beacon_node_info).await.map_err(|e| {
            error!("Failed to join the network: {}", e);
            e
        })?;
    }

    // Wrap the cache in a Mutex and Arc for safe sharing between tasks.
    let cache_ref = Arc::new(Mutex::new(cache));
    let node_ref = Arc::clone(&node);

    // Spawn background task to periodically save the cache file.
    if let Some(file_path) = config.cache_file_path.clone() {
        let node_for_saver = Arc::clone(&node_ref);
        let cache_for_saver = Arc::clone(&cache_ref);
        tokio::spawn(async move {
            let interval = Duration::from_secs(5);
            loop {
                // Fetch routing table from the node
                let all_contacts = node_for_saver.get_routing_table_content().await;

                // Lock the cache, update it, then save
                {
                    let mut locked_cache = cache_for_saver.lock().await;
                    locked_cache.routing_table_nodes = Some(all_contacts);
                    if let Err(e) = locked_cache.save_to_file(&file_path) {
                        error!("Failed to save cache file: {}", e);
                    }
                }
                time::sleep(interval).await;
            }
        });
    }

    // Spawn a task to handle shutdown signals.
    {
        let cache_ref_clone = Arc::clone(&cache_ref);
        let file_path_option = config.cache_file_path.clone();
        tokio::spawn(async move {
            // Wait for a Ctrl+C signal.
            if let Err(e) = tokio::signal::ctrl_c().await {
                error!("Failed to listen for Ctrl+C: {}", e);
            }
            info!("Received shutdown signal. Saving cache file...");

            // Save the cache one final time if a cache file path is specified.
            if let Some(file_path) = file_path_option {
                let cache_guard = cache_ref_clone.lock().await;
                if let Err(e) = cache_guard.save_to_file(&file_path) {
                    error!("Failed to save cache file during shutdown: {}", e);
                }
            }
            // Exit gracefully.
            std::process::exit(0);
        });
    }

    // Print the welcome message and available commands.
    println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    println!(
        "Welcome to the network! Your node is {}",
        node.to_node_info()
    );
    println!("Available commands:");
    println!(" - ping <key>: Send a PING request to the specified node");
    println!(" - find_node <key>: Resolves {} closest nodes to <key>", K);
    println!(" - upload <filepath>: Upload a file to the network");
    println!(" - download <file_handle> <storage_dir>: Download a file from the network");
    println!(" - dump_rt: Display the contents of the routing table");
    println!(" - dump_chunks: Display the chunks owned by this node");
    println!(
        " - Note: <key> should be a {}-character hexadecimal string",
        K * 2
    );
    println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");

    // Replace the synchronous stdin loop with an asynchronous one.
    let stdin = BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }

        match parts[0] {
            "ping" if parts.len() == 2 => match Key::from_hex_str(parts[1]) {
                Ok(node_id) => match node.ping(node_id).await {
                    Ok(response) => println!("{} responded to PING: {}", parts[1], response),
                    Err(e) => eprintln!("{} failed to respond to PING: {}", parts[1], e),
                },
                Err(e) => eprintln!("Invalid node ID: {}", e),
            },
            "find_node" if parts.len() == 2 => match Key::from_hex_str(parts[1]) {
                Ok(node_id) => match node.find_node(node_id).await {
                    Ok(closest_nodes) => {
                        println!("Closest nodes to {}:", parts[1]);
                        for node in closest_nodes {
                            println!(" - {}", node);
                        }
                    }
                    Err(e) => eprintln!("Failed to find closest nodes: {}", e),
                },
                Err(e) => eprintln!("Invalid node ID: {}", e),
            },
            "upload" if parts.len() == 2 => {
                let file_path = parts[1];
                match node.upload_file(file_path).await {
                    Ok(file_metadata) => {
                        println!("File uploaded successfully!");
                        println!("File handle: {}", file_metadata);
                    }
                    Err(e) => eprintln!("Failed to upload file: {}", e),
                }
            }
            "download" if parts.len() == 3 => {
                let file_handle = parts[1];
                let storage_dir = Path::new(parts[2]);
                match FileMetadata::from_str(file_handle) {
                    Ok(file_metadata) => {
                        match node.download_file(file_metadata, storage_dir).await {
                            Ok(file) => {
                                println!("File downloaded successfully! Saved to {:?}", file);
                            }
                            Err(e) => eprintln!("Failed to download file: {}", e),
                        }
                    }
                    Err(e) => eprintln!("Invalid file handle: {}", e),
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
                    "Wrong command or syntax '{}'. Valid commands are: 'dump_rt', 'find_node <key>', 'ping <key>', 'upload <filepath>', 'download <file_handle> <storage_dir>' or 'dump_chunks'",
                    parts[0]
                );
            }
        }
        println!("──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────");
    }

    Ok(())
}
