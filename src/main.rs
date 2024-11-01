mod utils;
mod cli;
mod crypto;
mod dht;
mod network;
mod sharding;
mod storage;

use clap::{Command, Arg};
use std::error::Error;

#[derive(Debug)]
struct Arguments {
    config: String,
    port: u16,
}

impl Arguments {
    fn new() -> Result<Self, Box<dyn Error>> {
        let matches = Command::new("Peer-to-peer System")
            .version("1.0.0")
            .author("TODO")
            .about("TODO")
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
            )
            .arg(
                Arg::new("port")
                    .short('p')
                    .long("port")
            )
            .get_matches();

        Ok(Self {
            config: matches.get_one::<String>("config").expect("Config is required").clone(),
            port: matches.get_one::<String>("port").unwrap().parse::<u16>().expect("Port is required").clone(),
        })
    }
}

fn main() {
    let config = Arguments::new().expect("Failed to parse CLI arguments");
    
    println!("{:?}", config);

    println!("Config file path: {}", config.config);
    println!("Port: {}", config.port);
}
