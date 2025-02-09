use super::encryption::AES_GCM_AUTH_TAG_SIZE_B;
use crate::constants::CHUNK_DATA_SIZE_KB;
use crate::core::key::Key as Hash;
use anyhow::{Context, Result};
use fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self};
use std::str::FromStr;
use thiserror::Error;

//pub const CHUNK_SIZE_KB_SMALL: usize = CHUNK_READ_KB_SMALL + AES_GCM_AUTH_TAG_SIZE; // KB
/// CHUNK_READ_KB * 1024 + 4 + AES_GCM_AUTH_TAG_SIZE_B
/// FIXME magic constant 4
pub const CHUNK_SIZE_B: usize = CHUNK_DATA_SIZE_KB * 1024 + 4 + AES_GCM_AUTH_TAG_SIZE_B; // BYTES

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
    #[error("Empty chunk")]
    EmptyChunk,
}

#[derive(Serialize, Deserialize)]
pub struct DecryptedChunkData {
    pub data_padded: Vec<u8>,
    pub data_unpadded_size: u32,
}

/// Encrypted or decrypted chunk
#[derive(Clone)]
pub struct Chunk {
    pub data: Vec<u8>, // Size of chunk data is also stored here
    pub hash: Hash,    // Hash of padded unencrypted chunk
                       //pub decrypted_data_unpadded_size: usize, // Real size of decrypted without possible padding
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkMetadata {
    pub hash: Hash,     // Hash of padded unencrypted chunk
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
