use std::collections::HashMap;
use std::time::SystemTime;
use tokio::sync::RwLock;

const DEFAULT_FILE_REUPLOAD_INTERVAL_S: u64 = 86400; // 24 h

pub struct StoredFileInfo {
    pub time_uploaded: SystemTime,
}

///
/// Keeps map of uploaded files to enable re-uploading them every fixed amount of time.
///
pub struct FileManager {
    uploaded_files: RwLock<HashMap<String, StoredFileInfo>>,
    reupload_interval_s: u64,
}

impl Default for FileManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FileManager {
    fn new_with_reupload_interval(reupload_interval_s: u64) -> FileManager {
        FileManager {
            uploaded_files: RwLock::new(HashMap::new()),
            reupload_interval_s,
        }
    }

    pub fn new() -> FileManager {
        FileManager::new_with_reupload_interval(DEFAULT_FILE_REUPLOAD_INTERVAL_S)
    }

    ///
    /// Add file to uploaded files map. This should be called when file is successfully
    /// uploaded to the network.
    ///
    pub async fn add_file_uploaded_entry(&self, full_file_path: &str) {
        let mut uploaded_files = self.uploaded_files.write().await;
        match uploaded_files.get_mut(full_file_path) {
            Some(file) => file.time_uploaded = SystemTime::now(),
            None => {
                uploaded_files.insert(
                    full_file_path.to_string(),
                    StoredFileInfo {
                        time_uploaded: SystemTime::now(),
                    },
                );
            }
        }
    }

    ///
    /// Get files that need to be re-uploaded and remove them from map (TTL = 0). Should be called
    /// periodically to re-upload files if the user has specified so.
    ///
    pub async fn get_files_for_reupload(&self) -> Vec<String> {
        let uploaded_files = self.uploaded_files.read().await;
        let reupload: Vec<String> = uploaded_files
            .iter()
            .filter(|f| f.1.time_uploaded.elapsed().unwrap().as_secs() >= self.reupload_interval_s)
            .map(|f| f.0.clone())
            .collect();

        reupload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_add_file_uploaded_entry() {
        let file_manager = FileManager::new();
        let file_path = "test_file.txt";

        file_manager.add_file_uploaded_entry(file_path).await;

        let uploaded_files = file_manager.uploaded_files.read().await;
        assert!(uploaded_files.contains_key(file_path));
    }

    #[tokio::test]
    async fn test_get_files_for_reupload() {
        let file_manager = FileManager::new_with_reupload_interval(0);
        let file_path = "test_file.txt";

        file_manager.add_file_uploaded_entry(file_path).await;

        let files_for_reupload = file_manager.get_files_for_reupload().await;
        assert!(files_for_reupload.contains(&file_path.to_string()));
    }

    #[tokio::test]
    async fn test_no_files_for_reupload_within_interval() {
        let file_manager = FileManager::new_with_reupload_interval(10); // 10 seconds interval
        let file_path = "test_file.txt";

        file_manager.add_file_uploaded_entry(file_path).await;
        sleep(Duration::from_secs(2)).await; // Wait for 2 seconds

        let files_for_reupload = file_manager.get_files_for_reupload().await;
        assert!(!files_for_reupload.contains(&file_path.to_string()));
    }
}
