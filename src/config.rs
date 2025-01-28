use crate::core::key::Key;
use get_if_addrs::get_if_addrs;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug)]
pub enum IpAddressType {
    Public,
    Local,
    Loopback,
}

impl fmt::Display for IpAddressType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddressType::Public => write!(f, "public"),
            IpAddressType::Local => write!(f, "local"),
            IpAddressType::Loopback => write!(f, "loopback"),
        }
    }
}

impl<'de> Deserialize<'de> for IpAddressType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "public" => Ok(IpAddressType::Public),
            "local" => Ok(IpAddressType::Local),
            "loopback" => Ok(IpAddressType::Loopback),
            _ => Err(serde::de::Error::custom("invalid IP address type")),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub beacon_node_address: Option<SocketAddr>,
    pub beacon_node_key: Option<Key>,
    /// Make this optional so we can prioritize the CLI port when provided.
    pub node_port: Option<u16>,
    pub ip_address_type: IpAddressType,
    pub cache_file_path: Option<String>,
    pub storage_path: String,
}

impl Config {
    /// - `file_path`: path to the config YAML
    /// - `skip_join`: if false, enforce that beacon_node_address and beacon_node_key are present
    /// - `port_arg`: an optional port argument from CLI (should override YAML if present)
    pub fn parse_from_file(
        file_path: &str,
        skip_join: bool,
        port_arg: Option<u16>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Read YAML from file
        let file = std::fs::File::open(file_path)?;
        let reader = std::io::BufReader::new(file);
        let mut config: Config = serde_yaml::from_reader(reader)?;

        // Validate paths
        config.validate_storage_path(&config.storage_path)?;
        if let Some(cache_file_path) = &config.cache_file_path {
            config.validate_cache_file_path(cache_file_path)?;
        }

        // If skip_join is false, these must be present:
        if !skip_join {
            if config.beacon_node_address.is_none() {
                return Err("beacon_node_address missing in config".into());
            }
            if config.beacon_node_key.is_none() {
                return Err("beacon_node_key missing in config".into());
            }
        }

        // Overwrite node_port from the command-line argument if provided
        if let Some(port) = port_arg {
            config.node_port = Some(port);
        }
        // If port is none in both the config and the CLI, pick ephemeral port
        if config.node_port.is_none() {
            config.node_port = Some(0);
        }

        Ok(config)
    }

    fn validate_storage_path(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(path);
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        } else if !path.is_dir() {
            return Err(format!("Storage path is not a directory: {}", path.display()).into());
        }
        Ok(())
    }

    fn validate_cache_file_path(&self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(path);
        if path.exists() {
            if path.is_file() {
                Ok(())
            } else {
                Err(format!("Cache path is not a file: {}", path.display()).into())
            }
        } else {
            Err(format!("Cache file does not exist: {}", path.display()).into())
        }
    }

    pub async fn resolve_ip_address(&self) -> Result<IpAddr, Box<dyn std::error::Error>> {
        match self.ip_address_type {
            IpAddressType::Public => {
                if let Some(ip) = public_ip::addr().await {
                    Ok(ip)
                } else {
                    Err("Failed to get public IP address".into())
                }
            }
            IpAddressType::Local => {
                let if_addrs = get_if_addrs()?;
                let local_ip = if_addrs
                    .into_iter()
                    .find(|iface| !iface.is_loopback() && iface.ip().is_ipv4())
                    .map(|iface| iface.ip())
                    .ok_or("Failed to get local IP address")?;
                Ok(local_ip)
            }
            IpAddressType::Loopback => Ok(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::net::{IpAddr, Ipv4Addr};
    use tempfile::tempdir;

    #[test]
    fn test_deserialize_config() {
        // node_port is optional, so in this test we want it to be set in the YAML
        let yaml = r#"
beacon_node_address: "127.0.0.1:1234"
beacon_node_key: "ffffffffffffffffffffffffffffffffffffffff"
node_port: 9999
ip_address_type: "local"
cache_file_path: "/tmp/cache"
storage_path: "/tmp/storage"
"#;

        let config: Config = serde_yaml::from_str(yaml).expect("Failed to deserialize Config");

        assert_eq!(
            config.beacon_node_address.unwrap(),
            "127.0.0.1:1234".parse().unwrap()
        );
        assert_eq!(
            config.beacon_node_key.as_ref().unwrap().to_hex_string(),
            "ffffffffffffffffffffffffffffffffffffffff"
        );
        // We changed node_port to an Option<u16>, so unwrap to compare
        assert_eq!(config.node_port.unwrap(), 9999);
        assert!(matches!(config.ip_address_type, IpAddressType::Local));
        assert_eq!(config.cache_file_path.as_ref().unwrap(), "/tmp/cache");
        assert_eq!(config.storage_path, "/tmp/storage");
    }

    #[test]
    fn test_parse_from_file() {
        // We'll supply skip_join = false so that beacon fields must be present.
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("storage");
        std::fs::create_dir_all(&storage_path).expect("Failed to create storage dir");

        let cache_file_path = temp_dir.path().join("cache_file");
        File::create(&cache_file_path).expect("Failed to create cache file");

        let yaml_data = format!(
            r#"
beacon_node_address: "127.0.0.1:5050"
beacon_node_key: "0000000000000000000000000000000000000000"
node_port: 8080
ip_address_type: "loopback"
cache_file_path: "{}"
storage_path: "{}"
"#,
            cache_file_path.display(),
            storage_path.display(),
        );

        let config_file_path = temp_dir.path().join("config.yaml");
        {
            let mut file = File::create(&config_file_path).unwrap();
            file.write_all(yaml_data.as_bytes()).unwrap();
        }

        // Pass skip_join = false, requiring beacon fields
        let config = Config::parse_from_file(config_file_path.to_str().unwrap(), false, None)
            .expect("Failed to parse config file");

        // Since skip_join = false, we expect the beacon_node_address is `Some(...)`.
        assert_eq!(
            config.beacon_node_address.unwrap(),
            "127.0.0.1:5050".parse().unwrap()
        );
        // We won’t check beacon_node_key specifically here, but it was present.
        // node_port is an Option, so let's unwrap it.
        assert_eq!(config.node_port.unwrap(), 8080);
        assert!(matches!(config.ip_address_type, IpAddressType::Loopback));
        assert_eq!(
            config.cache_file_path.as_ref().unwrap(),
            cache_file_path.to_str().unwrap()
        );
        assert_eq!(config.storage_path, storage_path.to_str().unwrap());
    }

    #[test]
    fn test_parse_from_file_with_port_override() {
        // Same setup, but we'll override node_port by passing Some(...) as port_arg
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().join("storage");
        std::fs::create_dir_all(&storage_path).expect("Failed to create storage dir");

        let cache_file_path = temp_dir.path().join("cache_file");
        File::create(&cache_file_path).expect("Failed to create cache file");

        let yaml_data = format!(
            r#"
beacon_node_address: "127.0.0.1:5050"
beacon_node_key: "0000000000000000000000000000000000000000"
node_port: 8888
ip_address_type: "loopback"
cache_file_path: "{}"
storage_path: "{}"
"#,
            cache_file_path.display(),
            storage_path.display(),
        );

        let config_file_path = temp_dir.path().join("config.yaml");
        {
            let mut file = File::create(&config_file_path).unwrap();
            file.write_all(yaml_data.as_bytes()).unwrap();
        }

        // We specify the port_arg override as Some(9999)
        let config = Config::parse_from_file(config_file_path.to_str().unwrap(), false, Some(9999))
            .expect("Failed to parse config file");

        // Even though the YAML has 8888, we should see 9999:
        assert_eq!(config.node_port.unwrap(), 9999);
    }

    #[tokio::test]
    async fn test_resolve_ip_address_public() {
        let config = Config {
            beacon_node_address: Some("127.0.0.1:3030".parse().unwrap()),
            beacon_node_key: Some(Key::new_random()),
            node_port: Some(3030),
            ip_address_type: IpAddressType::Public,
            cache_file_path: Some("/tmp/does_not_matter".to_string()),
            storage_path: "/tmp/does_not_matter".to_string(),
        };
        let ip = config.resolve_ip_address().await;
        assert!(ip.is_ok(), "Failed to resolve a public IP address");
    }

    #[tokio::test]
    async fn test_resolve_ip_address_local() {
        let config = Config {
            beacon_node_address: Some("127.0.0.1:3031".parse().unwrap()),
            beacon_node_key: Some(Key::new_random()),
            node_port: Some(3031),
            ip_address_type: IpAddressType::Local,
            cache_file_path: Some("/tmp/does_not_matter".to_string()),
            storage_path: "/tmp/does_not_matter".to_string(),
        };
        let ip = config.resolve_ip_address().await;
        assert!(ip.is_ok());
        assert!(ip.unwrap().is_ipv4());
    }

    #[tokio::test]
    async fn test_resolve_ip_address_loopback() {
        let config = Config {
            beacon_node_address: Some("127.0.0.1:3032".parse().unwrap()),
            beacon_node_key: Some(Key::new_random()),
            node_port: Some(3032),
            ip_address_type: IpAddressType::Loopback,
            cache_file_path: Some("/tmp/does_not_matter".to_string()),
            storage_path: "/tmp/does_not_matter".to_string(),
        };
        let ip = config.resolve_ip_address().await.unwrap();
        assert_eq!(ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }
}
