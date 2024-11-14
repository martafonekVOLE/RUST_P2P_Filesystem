use crate::config::KEY_LENGTH;
use rand::Rng;
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::fmt;

type KeyValue = [u8; KEY_LENGTH];

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct Key {
    pub(crate) value: KeyValue, // Assuming a 160-bit key for Kademlia
}

impl Key {
    /// Generate a new random key
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut value: KeyValue = [0u8; KEY_LENGTH];
        rng.fill(&mut value);
        Key { value }
    }

    /// Calculate a key from a given input (e.g., a GUID)
    pub fn from_input(input: &[u8]) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(input);
        let result = hasher.finalize();
        let mut value: KeyValue = [0u8; KEY_LENGTH];
        value.copy_from_slice(&result[..KEY_LENGTH]);
        Key { value }
    }

    /// Compare two keys using XOR
    pub fn distance(&self, other: &Key) -> KeyValue {
        let mut distance: KeyValue = [0u8; KEY_LENGTH];
        for (i, dist) in distance.iter_mut().enumerate().take(KEY_LENGTH) {
            *dist = self.value[i] ^ other.value[i];
        }
        distance
    }
}

// Implement Display for Key to print it in a readable format
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.value {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

// Implement Ord and PartialOrd for Key to compare keys
impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
