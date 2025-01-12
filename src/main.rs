use clap::{Arg, Command};
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
            .arg(Arg::new("config").short('c').long("config"))
            .arg(Arg::new("port").short('p').long("port"))
            .get_matches();

        Ok(Self {
            config: matches
                .get_one::<String>("config")
                .expect("Config is required")
                .clone(),
            port: matches
                .get_one::<String>("port")
                .unwrap()
                .parse::<u16>()
                .expect("Port is required")
                .clone(),
        })
    }
}

fn main() {
    let config = Arguments::new().expect("Failed to parse CLI arguments");

    println!("{:?}", config);

    println!("Config file path: {}", config.config);
    println!("Port: {}", config.port);
}

/*

* - required
^ - TRUE by default

^   -c --config: Config file with node's configuration. In YAML format.
        Contains:
            - beacon_node address: ip+port
            - node's port
            - cache file path: where to dump node's config when it turns off
            - storage path: where to store donwloaded files
    -b --beacon: Is it a beacond node? If so, beacon node's address is ignored if encountered in config file.
    --cache: Use the cache logic. Dump config to cache file when turned off.
    -v --verbose: Log info about the ongoing communication to stdout. (For debugging purposes)
    -h --help: Print help message
 */
