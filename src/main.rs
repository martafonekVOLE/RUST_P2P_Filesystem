mod cli;
mod config;

use clap::Parser;
use cli::args::Arguments;
use config::Config;
use std::fs;

fn main() {
    let args = Arguments::parse();

    let config_content = fs::read_to_string(&args.config).expect("Failed to read config file");
    let mut config: Config =
        serde_yaml::from_str(&config_content).expect("Failed to parse config file");

    if args.beacon {
        config.beacon_node_address = None;
    } else if config.beacon_node_address.is_none() {
        panic!("Beacon node address must be provided if not running as a beacon node");
    }

    println!("{:?}", args);
    println!("{:?}", config);
}


