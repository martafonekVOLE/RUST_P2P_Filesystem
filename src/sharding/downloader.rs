use super::common::*;
use super::encryption;
use crate::constants::CHUNK_DATA_SIZE_KB;
use std::path::{Path, PathBuf};
use tokio::fs::File as TokioFile;
use tokio::io::{AsyncWriteExt, BufWriter as TokioBufWriter};

pub struct FileDownloader {
    file_writer: TokioBufWriter<TokioFile>,
    file_metadata: FileMetadata,
    chunk_index: usize,
    output_file_path: PathBuf,
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
            output_file_path: file_path,
        })
    }

    /// TODO
    ///
    /// * `chunk_data` - byte slice containing padded chunk data and size of unpadded data
    pub async fn store_next_chunk_decrypt(&mut self, chunk: Chunk) -> Result<(), ShardingError> {
        // Check if chunk.data is empty
        if chunk.data.is_empty() {
            return Err(ShardingError::EmptyChunk);
        }

        if self.chunk_index >= self.file_metadata.chunks_metadata.len() {
            return Err(ShardingError::UnwantedChunk);
        }

        let chunk_metadata = &self.file_metadata.chunks_metadata[self.chunk_index];

        let mut decrypted_chunk_bytes = encryption::decrypt_payload(
            &chunk.data,
            &self.file_metadata.encryption_key,
            &chunk_metadata.nonce,
        )
        .map_err(|_| ShardingError::DecryptionFailed)?;

        // let mut chunk_data: DecryptedChunkData = bincode::deserialize(&decrypted_chunk_bytes)
        //     .map_err(|_| ShardingError::DecryptionFailed)?;

        let padded_chunk_size = CHUNK_DATA_SIZE_KB * 1024;
        assert_eq!(decrypted_chunk_bytes.len(), padded_chunk_size + 4);

        let unpadded_data_size = u32::from_le_bytes(
            decrypted_chunk_bytes[padded_chunk_size..padded_chunk_size + 4]
                .try_into()
                .unwrap(),
        );

        decrypted_chunk_bytes.truncate(padded_chunk_size);

        // TODO: don't need to compare the hash because AUTH TAG already ensures integrity
        if chunk.hash != chunk_metadata.hash {
            return Err(ShardingError::ChunkHashMismatch);
        }

        // Remove the padding if any
        if decrypted_chunk_bytes.len() != unpadded_data_size as usize {
            decrypted_chunk_bytes.truncate(unpadded_data_size as usize);
        }

        self.file_writer.write_all(&decrypted_chunk_bytes).await?;
        self.chunk_index += 1;
        if self.chunk_index >= self.file_metadata.chunks_metadata.len() {
            self.file_writer.flush().await?;
        }
        Ok(())
    }

    pub fn get_output_file_path(&self) -> &Path {
        &self.output_file_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::create_test_file_rng_filled;

    use crate::sharding::uploader::FileUploader;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_downloader_new() {
        let (file_path, _dir) = create_test_file_rng_filled(1024).await;
        let mut uploader = FileUploader::new(file_path.to_str().unwrap())
            .await
            .unwrap();
        uploader.get_next_chunk_encrypt().await.unwrap();
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
        let chunk = uploader.get_next_chunk_encrypt().await.unwrap().unwrap();
        let metadata = uploader.get_metadata().await.unwrap();
        let output_dir = tempdir().unwrap();
        let mut downloader = FileDownloader::new(metadata, output_dir.path())
            .await
            .unwrap();

        let result = downloader.store_next_chunk_decrypt(chunk).await;
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
        let chunk = uploader.get_next_chunk_encrypt().await.unwrap().unwrap();
        let metadata = uploader.get_metadata().await.unwrap();
        let output_dir = tempdir().unwrap();
        let mut downloader = FileDownloader::new(metadata, output_dir.path())
            .await
            .unwrap();

        // Simulate storing an extra chunk
        downloader.chunk_index = downloader.file_metadata.chunks_metadata.len();
        let result = downloader.store_next_chunk_decrypt(chunk).await;
        assert!(
            matches!(result, Err(ShardingError::UnwantedChunk)),
            "Expected ShardingError::UnwantedChunk, got: {:?}",
            result.err()
        );
    }
}
