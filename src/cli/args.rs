use clap::Parser;

#[derive(Parser, Debug)]
#[clap(name = "Peer-to-peer System", version = "1.0.0", author = "TODO")]
/// Peer-to-peer File Sharing System
pub struct Arguments {
    #[arg(short, long, required = true)]
    /// Config file with node's configuration. In YAML format.
    pub config: String,

    #[arg(short, long)]
    /// Is it a beacon node?
    pub beacon: bool,

    #[arg(long)]
    /// Use the cache logic. Dump config to cache file when turned off.
    pub cache: bool,

    #[arg(short, long)]
    /// Log info about the ongoing communication to stdout. (For debugging purposes).
    pub verbose: bool,
}

/*

* - required
^ - TRUE by default

^   -c --config: Config file with node's configuration. In YAML format.
        Contains:
            - beacon_node address: ip+port
            - node's port
            - cache file path: where to dump node's config when it turns off
            - storage path: where to store downloaded files
    -b --beacon: Is it a beacon node? If so, beacon node's address is ignored if encountered in config file.
    --cache: Use the cache logic. Dump config to cache file when turned off.
    -v --verbose: Log info about the ongoing communication to stdout. (For debugging purposes)
    -h --help: Print help message
 */

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_argument_parsing_with_config() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml"]);
        assert_eq!(args.config, "config.yaml");
        assert!(!args.beacon);
    }

    #[test]
    fn test_argument_parsing_with_beacon() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml", "--beacon"]);
        assert_eq!(args.config, "config.yaml");
        assert!(args.beacon);
    }

    #[test]
    fn test_argument_parsing_with_cache() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml", "--cache"]);
        assert_eq!(args.config, "config.yaml");
        assert!(args.cache);
    }

    #[test]
    fn test_argument_parsing_with_verbose() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml", "--verbose"]);
        assert_eq!(args.config, "config.yaml");
        assert!(args.verbose);
    }

    #[test]
    fn test_argument_parsing_missing_config() {
        let result = Arguments::try_parse_from(["test"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_argument_parsing_unknown_argument() {
        let result = Arguments::try_parse_from(["test", "--config", "config.yaml", "--unknown"]);
        assert!(result.is_err());
    }
}
