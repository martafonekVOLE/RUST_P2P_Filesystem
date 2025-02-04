use std::fs::File;

use serde::{Deserialize, Serialize};

use crate::{config::Config, core::key::Key};

#[derive(Debug, Serialize, Deserialize)]
pub struct Cache {
    pub key: Option<Key>,
    pub config: Config,
}

impl Cache {
    pub fn parse_from_file(file_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = std::fs::File::open(file_path)?;
        let reader = std::io::BufReader::new(file);
        let mut cache: Cache = serde_json::from_reader(reader)?;

        Ok(cache)
    }

    pub fn save_to_file(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::create(file_path)?;

        serde_json::to_writer_pretty(file, self)?;

        Ok(())
    }

    pub fn validate_cache_file_path(path: &str) -> Result<(), Box<dyn std::error::Error>> {
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
}
