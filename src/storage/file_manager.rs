use crate::core::key::Key;

pub struct FileManager {}

pub struct Chunk {
    hash: Key,
    chunk: Vec<u8>,
}

impl Chunk {
    pub fn get_hash(&self) -> Key {
        self.hash.clone()
    }
}

impl FileManager {
    pub fn check_file_exists(file_path: &str) -> bool {
        // Check if the provided file path exists in the storage path from Config
        todo!()
    }

    pub fn temp_sharding() -> Option<Chunk> {
        Some(Chunk {
            hash: Key::from_input(String::new().as_bytes()),
            chunk: Vec::new(),
        })
    }
}
