use crate::core::key::Key as Hash;
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use crate::sharding::common::{Chunk, CHUNK_READ_KB_LARGE, CHUNK_SIZE_KB_LARGE};
use crate::storage::data_transfers_table::{DataTransfer, DataTransfersTable};
use anyhow::{anyhow, bail, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

const MAX_SHARDS_STORED: usize = 1024;
const MAX_DATA_STORED_MB: usize = 4096; // 4 GB

const DEFAULT_CHUNK_REUPLOAD_INTERVAL_S: u64 = 3600; // 1 h
const DEFAULT_CHUNK_EXPIRE_TIME_S: u64 = 86400; // 24 h

pub struct StoredChunkInfo {
    pub time_stored_at: SystemTime,
}

impl StoredChunkInfo {
    pub fn update_time(&mut self, time: SystemTime) {
        self.time_stored_at = time;
    }
}

pub struct ShardStorageManager {
    storage_root_path: PathBuf,
    owned_chunks: HashMap<Hash, StoredChunkInfo>,
    data_transfers_table: DataTransfersTable,
    total_stored_kb: usize,
    reupload_interval_s: u64,
    expire_time_s: u64,
}

impl ShardStorageManager {
    pub fn new(storage_root_path: PathBuf) -> ShardStorageManager {
        // TODO: if storage_path is not empty, remove all contents?

        ShardStorageManager {
            storage_root_path,
            owned_chunks: HashMap::new(),
            data_transfers_table: DataTransfersTable::new(),
            total_stored_kb: 0,
            reupload_interval_s: DEFAULT_CHUNK_REUPLOAD_INTERVAL_S,
            expire_time_s: DEFAULT_CHUNK_EXPIRE_TIME_S,
        }
    }

    /// Save chunk received by TCP
    pub async fn store_chunk_for_known_peer(&mut self, data: Vec<u8>, port: u16) -> Result<()> {
        let data_transfer = self.data_transfers_table.get(port);
        let storage_root_path = self.storage_root_path.clone();

        match data_transfer {
            Some(data_transfer) => {
                if self.owned_chunks.len() >= MAX_SHARDS_STORED {
                    bail!("Max number of shards exceeded");
                }
                let chunk_size = data.len();

                if chunk_size > CHUNK_SIZE_KB_LARGE * 1024 {
                    bail!("Invalid chunk size");
                }
                if MAX_DATA_STORED_MB * 1024 - self.total_stored_kb < chunk_size {
                    bail!("Allocated storage memory exceeded. Can't store chunk");
                }

                let chunk_hash = data_transfer.chunk_hash;
                let chunk_full_path = storage_root_path.join(chunk_hash.to_string().as_str());
                if chunk_full_path.exists() {
                    bail!("Chunk is already stored. It was not necessary to download it!")
                }
                let mut file = File::create(chunk_full_path).await?;

                self.data_transfers_table.remove(port);

                file.write_all(&*data).await?;

                self.owned_chunks.insert(
                    chunk_hash,
                    StoredChunkInfo {
                        time_stored_at: SystemTime::now(),
                    },
                );
            }
            None => {
                bail!("Data transfer not found for this port. Unable to save file.");
            }
        };

        Ok(())
    }

    /// Read chunk from storage given its hash
    pub async fn read_chunk(&self, chunk_hash: &Hash) -> Result<Vec<u8>> {
        let chunk_full_path = self.storage_root_path.join(chunk_hash.to_string());
        if !chunk_full_path.exists() {
            bail!("Chunk not found in storage");
        }

        let data = fs::read(chunk_full_path).await?;
        Ok(data)
    }

    /// Removes expired chunks from storage drive. Must be called regularly by node.
    pub async fn remove_dead_chunks(&mut self) -> Result<()> {
        let dead: Vec<Hash> = self
            .owned_chunks
            .iter()
            .filter(|(_, info)| {
                info.time_stored_at.elapsed().unwrap().as_secs() >= self.expire_time_s
            })
            .map(|(hash, _)| hash.clone())
            .collect();

        // Delete all dead chunks from drive
        for hash in &dead {
            let chunk_full_path = self.storage_root_path.join(hash.to_string());
            if let Err(e) = fs::remove_file(chunk_full_path).await {
                bail!("Failed to delete chunk from drive: {}", e);
            }
        }

        // Remove all dead chunks from map
        self.owned_chunks.retain(|k, _| !dead.contains(k));

        Ok(())
    }

    /// Get chunks that must be reuploaded now. Must be called regularly by node.
    pub fn get_chunks_for_reupload(&mut self) -> Result<Vec<Hash>> {
        let reupload: Vec<Hash> = self
            .owned_chunks
            .iter()
            .filter(|(_, info)| {
                info.time_stored_at.elapsed().unwrap().as_secs() >= self.reupload_interval_s
            })
            .map(|(hash, _)| hash.clone())
            .collect();

        Ok(reupload)
    }

    /// Check if chunk is already owned.
    pub fn is_chunk_already_stored(&self, hash: &Hash) -> bool {
        self.owned_chunks.get(hash).is_some()
    }

    pub fn add_active_tcp_connection(
        &mut self,
        port: u16,
        sender: NodeInfo,
        chunk_hash: Hash,
    ) -> Option<DataTransfer> {
        self.data_transfers_table.add(port, sender, chunk_hash)
    }

    pub fn get_data_transfers_table(&self) -> &DataTransfersTable {
        &self.data_transfers_table
    }

    pub fn update_chunk_upload_time(&mut self, hash: &Hash) -> Result<()> {
        let mut chunk = self.owned_chunks.get_mut(hash);

        match chunk {
            Some(chunk_info) => {
                chunk_info.update_time(SystemTime::now());
                Ok(())
            }
            None => Err(anyhow!("Chunk not found in storage")),
        }
    }

    /// Hashes can then be used to read the chunk data and send it to node that is closer
    pub fn get_chunks_closer_to_node_than_to_me(
        &self,
        my_id: &Key,
        other_node_id: &Key,
    ) -> Vec<Hash> {
        self.owned_chunks
            .keys()
            .filter(|&chunk_hash| chunk_hash.distance(other_node_id) < chunk_hash.distance(my_id))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};
    use tempfile::tempdir;

    fn create_test_chunk() -> Chunk {
        Chunk {
            data: vec![0; CHUNK_SIZE_KB_LARGE],
            hash: Hash::new_random(),
            decrypted_data_unpadded_size: CHUNK_READ_KB_LARGE,
        }
    }

    #[tokio::test]
    async fn test_save_for_port() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let mut manager = ShardStorageManager::new(storage_path.clone());

        let chunk = create_test_chunk();
        let port = 8080;
        let sender = NodeInfo::new(Hash::new_random(), "127.0.0.1:8080".parse().unwrap());

        manager.add_active_tcp_connection(port, sender, chunk.hash.clone());

        let result = manager
            .store_chunk_for_known_peer(chunk.data.clone(), port)
            .await;
        assert!(result.is_ok());

        let chunk_path = storage_path.join(chunk.hash.to_string());
        assert!(chunk_path.exists());
    }

    #[tokio::test]
    async fn test_remove_dead_chunks() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let mut manager = ShardStorageManager::new(storage_path.clone());

        let chunk = create_test_chunk();
        let port = 8080;
        let sender = NodeInfo::new(Hash::new_random(), "127.0.0.1:8080".parse().unwrap());

        manager.add_active_tcp_connection(port, sender, chunk.hash.clone());
        manager
            .store_chunk_for_known_peer(chunk.data.clone(), port)
            .await
            .expect("Failed to save chunk");

        // Simulate chunk expiration
        manager
            .owned_chunks
            .get_mut(&chunk.hash)
            .unwrap()
            .time_stored_at = SystemTime::now() - Duration::from_secs(manager.expire_time_s + 1);

        let result = manager.remove_dead_chunks().await;
        assert!(result.is_ok());

        let chunk_path = Path::new(&storage_path).join(chunk.hash.to_string());
        assert!(!chunk_path.exists());
    }

    #[tokio::test]
    async fn test_get_chunks_for_reupload() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let mut manager = ShardStorageManager::new(storage_path.clone());

        let chunk = create_test_chunk();
        let port = 8080;
        let sender = NodeInfo::new(Hash::new_random(), "127.0.0.1:8080".parse().unwrap());

        manager.add_active_tcp_connection(port, sender, chunk.hash.clone());
        manager
            .store_chunk_for_known_peer(chunk.data.clone(), port)
            .await
            .expect("Failed to save chunk");

        // Simulate chunk needing reupload
        manager
            .owned_chunks
            .get_mut(&chunk.hash)
            .unwrap()
            .time_stored_at =
            SystemTime::now() - Duration::from_secs(manager.reupload_interval_s + 1);

        let chunks_for_reupload = manager
            .get_chunks_for_reupload()
            .expect("Failed to get chunks for reupload");
        assert_eq!(chunks_for_reupload.len(), 1);
        assert_eq!(chunks_for_reupload[0], chunk.hash);
    }

    #[tokio::test]
    async fn test_is_chunk_already_stored() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let mut manager = ShardStorageManager::new(storage_path.clone());

        let chunk = create_test_chunk();
        let port = 8080;
        let sender = NodeInfo::new(Hash::new_random(), "127.0.0.1:8080".parse().unwrap());

        manager.add_active_tcp_connection(port, sender, chunk.hash.clone());
        manager
            .store_chunk_for_known_peer(chunk.data.clone(), port)
            .await
            .expect("Failed to save chunk");

        assert!(manager.is_chunk_already_stored(&chunk.hash));
    }

    #[tokio::test]
    async fn test_read_chunk_success() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let mut manager = ShardStorageManager::new(storage_path.clone());

        let chunk = create_test_chunk();
        let port = 8080;
        let sender = NodeInfo::new(Hash::new_random(), "127.0.0.1:8080".parse().unwrap());

        manager.add_active_tcp_connection(port, sender, chunk.hash.clone());
        manager
            .store_chunk_for_known_peer(chunk.data.clone(), port)
            .await
            .expect("Failed to save chunk");

        let result = manager.read_chunk(&chunk.hash).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), chunk.data);
    }

    #[tokio::test]
    async fn test_read_chunk_not_found() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let storage_path = temp_dir.path().to_path_buf();
        let manager = ShardStorageManager::new(storage_path);

        let chunk_hash = Hash::new_random();
        let result = manager.read_chunk(&chunk_hash).await;
        assert!(result.is_err());
    }
}
