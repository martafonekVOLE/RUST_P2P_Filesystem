use crate::core::key::Key as Hash;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use thiserror::Error;
use tokio::fs::File as TokioFile;
use tokio::io::{
    AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader as TokioBufReader,
    BufWriter as TokioBufWriter,
};

use super::common::*;
use super::encryption; //::{ecrypt_payloaddecrypt_payload};
use super::uploader::FileUploader;
pub struct FileDownloader {
    file_writer: TokioBufWriter<TokioFile>,
    file_metadata: FileMetadata,
    chunk_index: usize,
}

impl FileDownloader {
    pub async fn new(
        file_metadata: FileMetadata,
        output_dir: &Path,
    ) -> Result<Self, ShardingError> {
        let file_path = output_dir.join(&file_metadata.name);
        let file = TokioFile::create(&file_path).await?;
        Ok(FileDownloader {
            file_writer: TokioBufWriter::new(file),
            file_metadata,
            chunk_index: 0,
        })
    }

    pub async fn store_next_chunk(&mut self, chunk: Chunk) -> Result<(), ShardingError> {
        if self.chunk_index >= self.file_metadata.chunks_metadata.len() {
            return Err(ShardingError::UnwantedChunk);
        }

        let chunk_metadata = &self.file_metadata.chunks_metadata[self.chunk_index];

        let mut decrypted_chunk_data = encryption::decrypt_payload(
            &chunk.data,
            &self.file_metadata.encryption_key,
            &chunk_metadata.nonce,
        )
        .map_err(|_| ShardingError::DecryptionFailed)?;

        // TODO: don't need to compare the hash because AUTH TAG already ensures integrity
        if chunk.hash != chunk_metadata.hash {
            return Err(ShardingError::ChunkHashMismatch);
        }

        // Remove the padding if any
        if decrypted_chunk_data.len() != chunk.decrypted_data_unpadded_size {
            decrypted_chunk_data.truncate(chunk.decrypted_data_unpadded_size);
        }

        self.file_writer.write_all(&decrypted_chunk_data).await?;
        self.chunk_index += 1;
        if self.chunk_index >= self.file_metadata.chunks_metadata.len() {
            self.file_writer.flush().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::create_test_file_rng_filled;

    use rand::Rng;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use tokio::fs::File as TokioFile;
    use tokio::io::AsyncWriteExt;

    #[tokio::test]
    async fn test_file_downloader_new() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        uploader.get_next_chunk().await.unwrap();
        let metadata = uploader.get_metadata().await.unwrap();
        let output_dir = tempdir().unwrap();
        let downloader = FileDownloader::new(metadata, output_dir.path()).await;

        assert!(
            downloader.is_ok(),
            "FileDownloader::new failed with error: {:?}",
            downloader.err()
        );
    }

    #[tokio::test]
    async fn test_file_downloader_store_next_chunk() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        let chunk = uploader.get_next_chunk().await.unwrap().unwrap();
        let metadata = uploader.get_metadata().await.unwrap();
        let output_dir = tempdir().unwrap();
        let mut downloader = FileDownloader::new(metadata, output_dir.path())
            .await
            .unwrap();

        let result = downloader.store_next_chunk(chunk).await;
        assert!(
            result.is_ok(),
            "FileDownloader::store_next_chunk failed with error: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_file_downloader_store_unwanted_chunk() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        let chunk = uploader.get_next_chunk().await.unwrap().unwrap();
        let metadata = uploader.get_metadata().await.unwrap();
        let output_dir = tempdir().unwrap();
        let mut downloader = FileDownloader::new(metadata, output_dir.path())
            .await
            .unwrap();

        // Simulate storing an extra chunk
        downloader.chunk_index = downloader.file_metadata.chunks_metadata.len();
        let result = downloader.store_next_chunk(chunk).await;
        assert!(
            matches!(result, Err(ShardingError::UnwantedChunk)),
            "Expected ShardingError::UnwantedChunk, got: {:?}",
            result.err()
        );
    }
}
