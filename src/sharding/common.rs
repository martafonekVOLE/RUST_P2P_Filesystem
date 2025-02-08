use super::encryption::AES_GCM_AUTH_TAG_SIZE;
use crate::core::key::Key as Hash;
use anyhow::{Context, Result};
use fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self};
use std::str::FromStr;
use thiserror::Error;

// Actual size of the chunk will be 16 bytes longer
// because of auth tag on encyrpted message.
// These constants just represent how much will be read from file to fill the chunk
pub const CHUNK_READ_KB_SMALL: usize = 512; // KB
pub const CHUNK_READ_KB_LARGE: usize = 1024; // KB

pub const CHUNK_SIZE_KB_SMALL: usize = CHUNK_READ_KB_SMALL + AES_GCM_AUTH_TAG_SIZE; // KB
pub const CHUNK_SIZE_KB_LARGE: usize = CHUNK_READ_KB_LARGE + AES_GCM_AUTH_TAG_SIZE; // KB

pub const MAX_FILE_SIZE_MB: usize = 4096; // 4 GB
pub const LARGE_FILE_THRESHOLD_MB: usize = 1024; // 1 GB

#[derive(Error, Debug)]
pub enum ShardingError {
    #[error("IO error")]
    Io(#[from] io::Error),
    #[error("File is too big to be uploaded to the network")]
    FileTooBig,
    #[error("File was not read completely")]
    MetadataNotFilled,
    #[error("Failed to encrypt chunk")]
    EncryptionFailed,
    #[error("Failed to decrypt chunk")]
    DecryptionFailed,
    #[error("Unwanted chunk. Metadata for chunk not found")]
    UnwantedChunk,
    #[error("Hash mismatch between metadata and chunk")]
    ChunkHashMismatch,
}

#[derive(Clone)]
pub struct Chunk {
    pub data: Vec<u8>,
    pub hash: Hash, // TODO: maybe don't need to pass to external modules,
                    // because no need to verify it, AEAD auth tag already ensures integrity
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkMetadata {
    pub hash: Hash,
    pub nonce: Vec<u8>, // Is required to decrypt the message. 12-byte long unique value.
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileMetadata {
    pub name: String,
    pub size: usize, // E.g. to know which chunk size to expect
    pub encryption_key: Vec<u8>,
    pub chunks_metadata: Vec<ChunkMetadata>,
}

impl Display for FileMetadata {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // Serialize self using bincode
        match bincode::serialize(self) {
            Ok(bytes) => write!(f, "{}", hex::encode(bytes)),
            Err(e) => write!(f, "Serialization error: {}", e),
        }
    }
}

impl FromStr for FileMetadata {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        // Decode the hex string back into bytes.
        let bytes = hex::decode(s).context("Failed to decode hex string")?;
        // Deserialize the bytes back into a FileMetadata.
        let metadata =
            bincode::deserialize(&bytes).context("Failed to deserialize FileMetadata")?;
        Ok(metadata)
    }
}
