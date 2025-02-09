use super::encryption::{EncryptionError, AES_GCM_AUTH_TAG_SIZE_B};
use crate::constants::CHUNK_DATA_SIZE_KB;
use crate::core::key::Key as Hash;
use anyhow::{Context, Result};
use fmt::{Display, Formatter};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{self};
use std::str::FromStr;
use thiserror::Error;

pub const U32_SIZE_B: usize = 4;
/// Total size of encrypted chunk
pub const ENCRYPTED_CHUNK_SIZE_B: usize =
    CHUNK_DATA_SIZE_KB * 1024 + U32_SIZE_B + AES_GCM_AUTH_TAG_SIZE_B;

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
    #[error("Chunk encryption failed")]
    EncryptionError(#[from] EncryptionError),
}

/// Encrypted or unencrypted chunk.
#[derive(Clone)]
pub struct Chunk {
    /// Binary payload of chunk, includes padded byte array and size of unpadded data.
    pub data: Vec<u8>,
    /// Hash calculated from unencrypted padded data (that was read from file).
    pub hash: Hash,
}

/// Metadata of chunk, stored in metadata file, used to download and decrypt the chunk.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChunkMetadata {
    /// Hash of padded unencrypted chunk binary data.
    pub hash: Hash,
    /// Is required to decrypt the message. 12-byte long unique value.
    pub nonce: Vec<u8>,
}

/// Metadata, required to download a file.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileMetadata {
    /// Name of the file.
    pub name: String,
    /// AEAD unique encryption key.
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
