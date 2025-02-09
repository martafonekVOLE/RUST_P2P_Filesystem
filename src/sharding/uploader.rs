use super::common::*;
use super::encryption;
use crate::constants::{CHUNK_DATA_SIZE_KB, MAX_FILE_SIZE_MB};
use crate::core::key::Key as Hash;
use rand::Rng;
use std::path::Path;
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader as TokioBufReader};

pub struct FileUploader {
    file_reader: TokioBufReader<TokioFile>,
    file_metadata: FileMetadata,
    encryption_key: Vec<u8>,
    //chunk_size: usize,
}

impl FileUploader {
    // For testing purposes
    async fn new_with_max_file_size(
        full_file_path: &str,
        max_file_size_mb: usize,
    ) -> Result<Self, ShardingError> {
        let full_file_path = Path::new(full_file_path);
        let file = if full_file_path.exists() {
            TokioFile::open(full_file_path).await?
        } else {
            TokioFile::create(full_file_path).await?
        };
        let metadata = file.metadata().await?;
        let file_size = metadata.len() as usize;
        if file_size > max_file_size_mb * 1024 * 1024 {
            return Err(ShardingError::FileTooBig);
        }

        //let chunk_size = Self::get_chunk_size_from_file_size(file_size);
        //let chunk_size = CHUNK_READ_KB_LARGE;
        let encryption_key = encryption::generate_key();
        let current_file_md = FileMetadata {
            name: full_file_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            encryption_key: encryption_key.to_vec(),
            chunks_metadata: Vec::new(),
        };

        Ok(FileUploader {
            file_reader: TokioBufReader::new(file),
            file_metadata: current_file_md,
            encryption_key,
            //chunk_size,
        })
    }

    pub async fn new(full_file_path: &str) -> Result<Self, ShardingError> {
        FileUploader::new_with_max_file_size(full_file_path, MAX_FILE_SIZE_MB).await
    }

    // fn get_chunk_size_from_file_size(file_size: usize) -> usize {
    //     if file_size < LARGE_FILE_THRESHOLD_MB * 1024 * 1024 {
    //         CHUNK_READ_KB_SMALL * 1024
    //     } else {
    //         CHUNK_READ_KB_LARGE * 1024
    //     }
    // }

    pub async fn get_next_chunk_encrypt(&mut self) -> Result<Option<Chunk>, ShardingError> {
        let file = &mut self.file_reader;
        let padded_size = CHUNK_DATA_SIZE_KB * 1024;
        let mut padded_data_buffer = vec![0; padded_size];
        let bytes_read = file.read(&mut padded_data_buffer).await?;
        if bytes_read == 0 {
            return Ok(None);
        }
        padded_data_buffer.truncate(bytes_read);

        if bytes_read < padded_size {
            // Add padding with random bytes
            let mut rng = rand::thread_rng();
            let random_bytes: Vec<u8> = (0..padded_size - bytes_read)
                .map(|_| rng.random())
                .collect();
            padded_data_buffer.extend(&random_bytes);
        }
        debug_assert_eq!(padded_data_buffer.len(), padded_size);

        let unpadded_size_bytes = (bytes_read as u32).to_le_bytes();
        debug_assert_eq!(unpadded_size_bytes.len(), 4);

        let chunk_hash = Hash::from_input(&padded_data_buffer);
        padded_data_buffer.extend_from_slice(&unpadded_size_bytes);

        let (nonce, encrypted_chunk) =
            encryption::encrypt_payload(&padded_data_buffer, &self.encryption_key)?;

        let chunk = Chunk {
            data: encrypted_chunk.to_vec(),
            hash: chunk_hash, // Hash of unencrypted chunk
        };

        self.file_metadata.chunks_metadata.push(ChunkMetadata {
            hash: chunk.hash,
            nonce: nonce.to_vec(),
        });

        Ok(Some(chunk))
    }

    pub async fn get_metadata(mut self) -> Result<FileMetadata, ShardingError> {
        if self.file_reader.fill_buf().await?.is_empty() {
            Ok(self.file_metadata)
        } else {
            Err(ShardingError::MetadataNotFilled)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::testing::{create_test_file_rng_filled, create_test_file_zero_filled};

    #[tokio::test]
    async fn test_file_uploader_new() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let uploader = FileUploader::new(file_path.to_str().unwrap()).await;

        assert!(
            uploader.is_ok(),
            "FileUploader::new failed with error: {:?}",
            uploader.err()
        );
    }

    #[tokio::test]
    async fn test_file_uploader_file_too_big() {
        let max_file_size_mb = 1;
        let (file_path, _dir) =
            create_test_file_zero_filled(max_file_size_mb * 1024 * 1024 + 1).await;
        let uploader =
            FileUploader::new_with_max_file_size(file_path.to_str().unwrap(), max_file_size_mb)
                .await;
        assert!(
            matches!(uploader, Err(ShardingError::FileTooBig)),
            "Expected ShardingError::FileTooBig, got: {:?}",
            uploader.err()
        );
    }

    #[tokio::test]
    async fn test_file_uploader_get_next_chunk() {
        let file_size = 1024; // size smaller than chunk size!
        let (file_path, _dir) = create_test_file_rng_filled(file_size).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        let chunk = uploader.get_next_chunk_encrypt().await.unwrap();
        assert!(chunk.is_some(), "Expected Some(chunk), got None");
        assert_eq!(
            chunk.unwrap().data.len(),
            ENCRYPTED_CHUNK_SIZE_B,
            "Chunk length mismatch"
        );

        let chunk = uploader.get_next_chunk_encrypt().await.unwrap();
        assert!(chunk.is_none(), "Expected None");
    }

    #[tokio::test]
    async fn test_file_uploader_get_metadata() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        let chunk = uploader.get_next_chunk_encrypt().await.unwrap();
        assert!(chunk.is_some(), "Expected Some(chunk), got None");
        let metadata = uploader.get_metadata().await;
        assert!(
            metadata.is_ok(),
            "Error: get_metadata failed with error: {:?}",
            metadata.err()
        );
    }

    #[tokio::test]
    async fn test_file_uploader_get_metadata_fail() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        let metadata = uploader.get_metadata().await;
        assert!(
            metadata.is_err(),
            "Expected get_metadata to fail cause file was not read completely"
        );
    }
}
