use crate::core::key::Key;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use thiserror::Error;

pub const CHUNK_SIZE_KB_SMALL: usize = 512; // KB
pub const CHUNK_SIZE_KB_LARGE: usize = 1024; // KB

pub const MAX_FILE_SIZE_MB: usize = 4096; // 4 GB
pub const LARGE_FILE_THRESHOLD_MB: usize = 1024; // 1 GB

#[derive(Error, Debug)]
pub enum ShardingError {
    #[error("IO error")]
    Io(#[from] io::Error),
    #[error("File is too big to be uploaded to the network")]
    FileTooBig,
}

pub struct ChunkBytes {
    data: Vec<u8>,
    hash: Key,
}

pub struct FileProcessed {
    //metadata: FileMetadata,
    chunks: Vec<ChunkBytes>,
}

pub struct FileBytes {
    bytes: Vec<u8>,
}

/// The order of chunks must be ensured by receiver
pub struct ChunkMetadata {
    hash: Key,   // Hash of an encrypted chunk (to FIND_VALUE and validate integrity)
    size: usize, // (!) May be needed to know how much to receive from peer over TCP
}

/// Metadata received somewhere from network, read from file

#[derive(Debug, Deserialize)]
//#[serde(rename_all = "PascalCase")]
pub struct FileMetadata {
    name: String,
    size: usize, // To know if file is actually small enough to be allowed for download
    hash: Key,   // TODO: may be redundant? Might be useful to speed-up integrity check
    chunk_hashes: Vec<Key>,
}

pub fn read_file_metadata(file_path: &str) -> Result<FileMetadata, ShardingError> {
    let file_md: FileMetadata =
        serde_json::from_str(file_path).expect("JSON was not well-formatted");
    Ok(file_md)
}

pub fn process_file_for_upload(file_path: &Path) -> Result<FileProcessed, ShardingError> {
    let file = File::open(file_path)?;
    let metadata = file.metadata()?;
    let file_size = metadata.len() as usize;
    //let mut buffer = vec![0; file_size];
    if file_size > MAX_FILE_SIZE_MB * 1024 * 1024 {
        return Err(ShardingError::FileTooBig);
    }

    let chunks = split_file_in_chunks(&file)?;

    Ok(FileProcessed {
        //metadata: file_metadata,
        chunks,
    })
}

/// Combines chunks into a single byte array of file data.
///
/// At this point chunks are already verified by hash.
/// TODO: Encryption: will chunks be encrypted separately? Otherwise need to hold whole file in RAM to decrypt.
pub fn get_file_from_chunks(chunks: Vec<ChunkBytes>) -> Result<FileBytes, ShardingError> {
    let mut file_bytes = Vec::<u8>::new();

    for chunk in chunks {
        file_bytes.extend_from_slice(&chunk.data);
    }

    Ok(FileBytes { bytes: file_bytes })
}

fn split_file_in_chunks(mut file: &File) -> Result<Vec<ChunkBytes>, ShardingError> {
    let metadata = file.metadata()?;
    let file_size = metadata.len() as usize;

    let chunk_size = if file_size > LARGE_FILE_THRESHOLD_MB * 1024 * 1024 {
        CHUNK_SIZE_KB_LARGE * 1024
    } else {
        CHUNK_SIZE_KB_SMALL * 1024
    };

    let mut chunks = Vec::new();
    let mut buffer = vec![0; chunk_size];

    while let Ok(bytes_read) = file.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        // (!) Last chunk may be smaller than chunk size. Shouldn't cause any problem, verified by hash
        let chunk = ChunkBytes {
            data: buffer[..bytes_read].to_vec(),
            hash: Key::from_input(&buffer[..bytes_read]),
        };
        //chunk.data.extend_from_slice(&buffer[..bytes_read]);
        chunks.push(chunk);
    }

    Ok(chunks)
}
