pub const K: usize = 20;
pub const ALPHA: usize = 3;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub beacon_node_address: Option<String>,
    pub node_port: u16,
    pub cache_file_path: String,
    pub storage_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_config_parsing() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("config.yaml");
        let mut file = File::create(&file_path).unwrap();

        writeln!(
            file,
            r#"
beacon_node_address: "127.0.0.1:8080"
node_port: 8081
cache_file_path: "cache.yaml"
storage_path: "storage"
"#
        )
        .unwrap();

        let config_content = fs::read_to_string(&file_path).expect("Failed to read config file");
        let config: Config =
            serde_yaml::from_str(&config_content).expect("Failed to parse config file");

        assert_eq!(
            config.beacon_node_address,
            Some("127.0.0.1:8080".to_string())
        );
        assert_eq!(config.node_port, 8081);
        assert_eq!(config.cache_file_path, "cache.yaml".to_string());
        assert_eq!(config.storage_path, "storage".to_string());
    }
}
