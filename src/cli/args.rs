use clap::{ArgGroup, Parser};

#[derive(Parser, Debug)]
#[clap(name = "Peer-to-peer file sharing system", version = "1.0.0")]
#[clap(group(
    ArgGroup::new("input")
        .required(true)
        .args(&["config", "cache"])
))]
/// Peer-to-peer file sharing system written in Rust.
pub struct Arguments {
    #[arg(short, long, required = true)]
    /// Config file with node's configuration. In YAML format.
    pub config: Option<String>,

    #[arg(long)]
    /// Path to the cached node file. This is used to relaunch stopped nodes.
    pub cache: Option<String>,

    #[arg(short, long)]
    /// Log info about the ongoing communication to stdout. (For debugging purposes).
    pub verbose: bool,

    #[arg(short, long)]
    /// Run without beacon node.
    pub skip_join: bool,

    #[arg(short, long)]
    /// The port to run the node on. If not provided, a random port will be chosen. This overrides
    /// the port specified in the config file.
    pub port: Option<u16>,

    #[arg(short, long)]
    /// Creates the storage directory automatically based on this nodes key.
    /// Overrides the storage path specified in the config file.
    pub automatic_storage: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_argument_parsing_with_config() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml"]);
        // Since config is an Option, we use as_deref() to compare the inner &str.
        assert_eq!(args.config.as_deref(), Some("config.yaml"));
        // cache was not provided so it should be None.
        assert!(args.cache.is_none());
    }

    #[test]
    fn test_argument_parsing_with_cache() {
        let args = Arguments::parse_from(["test", "--cache", "cache.json"]);
        // When only cache is provided, config remains None.
        assert_eq!(args.cache.as_deref(), Some("cache.json"));
        assert!(args.config.is_none());
    }

    #[test]
    fn test_argument_parsing_with_verbose() {
        let args = Arguments::parse_from(["test", "--config", "config.yaml", "--verbose"]);
        assert_eq!(args.config.as_deref(), Some("config.yaml"));
        assert!(args.verbose);
    }

    #[test]
    fn test_argument_parsing_missing_input() {
        // Neither --config nor --cache provided: error expected.
        let result = Arguments::try_parse_from(["test"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_argument_parsing_unknown_argument() {
        let result = Arguments::try_parse_from(["test", "--config", "config.yaml", "--unknown"]);
        assert!(result.is_err());
    }
}
