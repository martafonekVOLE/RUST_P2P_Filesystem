use std::{
    fs::{create_dir_all, File},
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{config::Config, core::key::Key, networking::node_info::NodeInfo};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Cache {
    pub key: Key,
    pub skip_join: bool,
    pub config: Config,
    pub routing_table_nodes: Option<Vec<NodeInfo>>,
}

impl Cache {
    pub fn parse_from_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(file_path)?;
        let reader = std::io::BufReader::new(file);
        let cache: Cache = serde_json::from_reader(reader)?;

        Ok(cache)
    }

    pub fn save_to_file(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = Path::new(file_path).parent() {
            if !parent.exists() {
                create_dir_all(parent)?;
            }
        }

        let file = File::create(file_path)?;
        serde_json::to_writer_pretty(file, self)?;
        Ok(())
    }
}
