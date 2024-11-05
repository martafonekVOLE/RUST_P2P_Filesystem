use rand::Rng;
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct Key {
    pub(crate) value: [u8; 20], // Assuming a 160-bit key for Kademlia
}

impl Key {
    /// Generate a new random key
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut value = [0u8; 20];
        rng.fill(&mut value);
        Key { value }
    }

    /// Calculate a key from a given input (e.g., a GUID)
    pub fn from_input(input: &[u8]) -> Self {
        use sha1::{Digest, Sha1};
        let mut hasher = Sha1::new();
        hasher.update(input);
        let result = hasher.finalize();
        let mut value = [0u8; 20];
        value.copy_from_slice(&result[..20]);
        Key { value }
    }

    /// Compare two keys using XOR
    pub fn distance(&self, other: &Key) -> [u8; 20] {
        let mut distance = [0u8; 20];
        for i in 0..20 {
            distance[i] = self.value[i] ^ other.value[i];
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
