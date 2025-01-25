use get_if_addrs::get_if_addrs;
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::net::IpAddr;

#[derive(Debug)]
pub enum IpAddress {
    Public,
    Local,
    Loopback,
}

impl fmt::Display for IpAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddress::Public => write!(f, "public"),
            IpAddress::Local => write!(f, "local"),
            IpAddress::Loopback => write!(f, "loopback"),
        }
    }
}

impl<'de> Deserialize<'de> for IpAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "public" => Ok(IpAddress::Public),
            "local" => Ok(IpAddress::Local),
            "loopback" => Ok(IpAddress::Loopback),
            _ => Err(serde::de::Error::custom("invalid IP address type")),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Config {
    ///
    pub beacon_node_address: String,
    pub node_port: u16,
    pub cache_file_path: String,
    pub storage_path: String,
    pub ip_address: IpAddress,
}

impl Config {
    pub fn new(
        beacon_node_address: String,
        node_port: u16,
        cache_file_path: String,
        storage_path: String,
        ip_address: IpAddress,
    ) -> Self {
        Config {
            beacon_node_address,
            node_port,
            cache_file_path,
            storage_path,
            ip_address,
        }
    }

    pub fn read_from_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(file_path)?;
        let reader = std::io::BufReader::new(file);
        let config = serde_yaml::from_reader(reader)?;
        Ok(config)
    }

    pub async fn resolve_ip_address(&self) -> Result<IpAddr, Box<dyn std::error::Error>> {
        match self.ip_address {
            IpAddress::Public => {
                if let Some(ip) = public_ip::addr().await {
                    Ok(ip)
                } else {
                    Err("Failed to get public IP address".into())
                }
            }
            IpAddress::Local => {
                let if_addrs = get_if_addrs()?;
                let local_ip = if_addrs
                    .into_iter()
                    .find(|iface| !iface.is_loopback() && iface.ip().is_ipv4())
                    .map(|iface| iface.ip())
                    .ok_or("Failed to get local IP address")?;
                Ok(local_ip)
            }
            IpAddress::Loopback => Ok(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::net::IpAddr;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_resolve_ip_address_public() {
        let config = Config {
            beacon_node_address: "http://example.com".to_string(),
            node_port: 8080,
            cache_file_path: "/tmp/cache".to_string(),
            storage_path: "/tmp/storage".to_string(),
            ip_address: IpAddress::Public,
        };

        // Skip testing actual public IP resolution in CI or tests
        let result = config.resolve_ip_address().await;
        assert!(result.is_ok(), "Public IP resolution failed");
    }

    #[tokio::test]
    async fn test_resolve_ip_address_local() {
        let config = Config {
            beacon_node_address: "http://example.com".to_string(),
            node_port: 8080,
            cache_file_path: "/tmp/cache".to_string(),
            storage_path: "/tmp/storage".to_string(),
            ip_address: IpAddress::Local,
        };

        let result = config.resolve_ip_address().await;
        assert!(result.is_ok(), "Local IP resolution failed");
        assert!(result.unwrap().is_ipv4(), "Expected an IPv4 address");
    }

    #[tokio::test]
    async fn test_resolve_ip_address_loopback() {
        let config = Config {
            beacon_node_address: "http://example.com".to_string(),
            node_port: 8080,
            cache_file_path: "/tmp/cache".to_string(),
            storage_path: "/tmp/storage".to_string(),
            ip_address: IpAddress::Loopback,
        };

        let result = config.resolve_ip_address().await;
        assert!(result.is_ok(), "Loopback IP resolution failed");
        assert_eq!(result.unwrap(), IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn test_config_deserialization() {
        let yaml_data = r#"
        beacon_node_address: "http://example.com"
        node_port: 8080
        cache_file_path: "/tmp/cache"
        storage_path: "/tmp/storage"
        ip_address: "local"
        "#;

        let config: Config = serde_yaml::from_str(yaml_data).expect("Deserialization failed");
        assert_eq!(config.beacon_node_address, "http://example.com");
        assert_eq!(config.node_port, 8080);
        assert_eq!(config.cache_file_path, "/tmp/cache");
        assert_eq!(config.storage_path, "/tmp/storage");
        assert!(matches!(config.ip_address, IpAddress::Local));
    }

    #[test]
    fn test_read_from_file() {
        let yaml_data = r#"
        beacon_node_address: "http://example.com"
        node_port: 8080
        cache_file_path: "/tmp/cache"
        storage_path: "/tmp/storage"
        ip_address: "loopback"
        "#;

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("config.yaml");

        let mut file = File::create(&file_path).expect("Failed to create file");
        file.write_all(yaml_data.as_bytes())
            .expect("Failed to write to file");

        let config = Config::read_from_file(file_path.to_str().unwrap())
            .expect("Failed to read config from file");

        assert_eq!(config.beacon_node_address, "http://example.com");
        assert_eq!(config.node_port, 8080);
        assert_eq!(config.cache_file_path, "/tmp/cache");
        assert_eq!(config.storage_path, "/tmp/storage");
        assert!(matches!(config.ip_address, IpAddress::Loopback));
    }

    #[test]
    fn test_ip_address_deserialization() {
        let public_ip: IpAddress =
            serde_yaml::from_str("\"public\"").expect("Deserialization failed");
        assert!(matches!(public_ip, IpAddress::Public));

        let local_ip: IpAddress =
            serde_yaml::from_str("\"local\"").expect("Deserialization failed");
        assert!(matches!(local_ip, IpAddress::Local));

        let loopback_ip: IpAddress =
            serde_yaml::from_str("\"loopback\"").expect("Deserialization failed");
        assert!(matches!(loopback_ip, IpAddress::Loopback));

        let invalid_ip: Result<IpAddress, _> = serde_yaml::from_str("\"invalid\"");
        assert!(invalid_ip.is_err(), "Expected an error for invalid IP type");
    }
}
