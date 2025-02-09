use rand::Rng;
use std::path::PathBuf;
use tempfile::tempdir;
use tokio::fs::File as TokioFile;
use tokio::io::AsyncWriteExt;

async fn create_test_file(
    maybe_size: Option<usize>,
    randomly: bool,
) -> (PathBuf, tempfile::TempDir) {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_file");
    let mut file = TokioFile::create(&file_path).await.unwrap();
    let mut rng = rand::thread_rng();
    if let Some(size) = maybe_size {
        let bytes: Vec<u8> = if randomly {
            (0..size).map(|_| rng.random()).collect()
        } else {
            vec![0u8; size]
        };
        file.write_all(&bytes).await.unwrap();
    }
    (file_path, dir)
}

pub async fn create_test_file_rng_filled(size: usize) -> (PathBuf, tempfile::TempDir) {
    create_test_file(size.into(), true).await
}

pub async fn create_test_file_zero_filled(size: usize) -> (PathBuf, tempfile::TempDir) {
    create_test_file(size.into(), true).await
}

pub async fn create_test_file_empty() -> (PathBuf, tempfile::TempDir) {
    create_test_file(None, false).await
}

/// Converts any given path to a Unix-style path
pub fn to_unix_path(path: String) -> String {
    path.replace("\\", "/")
}
