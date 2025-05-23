use crate::constants::K;
use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha1::{Digest, Sha1};
use std::cmp::Ordering;
use std::fmt;
use std::fmt::LowerHex;
use thiserror::Error;

///
/// Represents an error that can occur when constructing or parsing a `Key`.
///
#[derive(Debug, Error)]
pub enum KeyError {
    #[error("Invalid length of decoded hex bytes: got {0}")]
    InvalidHexLength(usize),

    #[error("Failed to decode hex: {0}")]
    DecodeHexError(#[from] hex::FromHexError),

    #[error("Hex string length must be {expected} but got {got}")]
    WrongHexStringLength { expected: usize, got: usize },
}

///
/// A fixed-size byte array for the key. The size is defined by the `K` constant - this comes from
/// the Kademlia paper and is typically 20 bytes in production systems.
///
pub type KeyValue = [u8; K];

///
/// # The main identifier of the Kademlia DHT
///
/// A key representing a 160-bit (with `K` = 20) identifier of either a node or a data chunk.
/// This is the core type used in the Kademlia DHT. The file chunks and the nodes in the network
/// share the same key space, which allows collisions between chunk and node, but collisions pairs
/// of chunks or pairs of nodes are not allowed. Given a large enough key space, the probability of
/// collisions is negligible.
///
#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct Key {
    value: KeyValue,
}

impl Key {
    ///
    /// Generates a new random `Key` using thread-local RNG.
    ///
    /// Uses thread_rng which should provide good entropy for key generation.
    ///
    pub fn new_random() -> Self {
        let mut rng = rand::thread_rng();
        let mut value: KeyValue = [0u8; K];
        rng.fill(&mut value);
        Key { value }
    }

    ///
    /// Constructs a `Key` from exactly 20 bytes (if `K` is 20).
    ///
    /// This is a convenience function if you already have the bytes.
    ///
    pub fn from_bytes(bytes: KeyValue) -> Self {
        Key { value: bytes }
    }

    ///
    /// Creates a key from a hexadecimal string of length `2 * K` characters.
    ///
    /// This is useful for parsing keys from configuration files or other inputs.
    ///
    /// Returns `KeyError` if the length or decoding is invalid.
    ///
    pub fn from_hex_str(hex_str: &str) -> Result<Self, KeyError> {
        if hex_str.len() != K * 2 {
            return Err(KeyError::WrongHexStringLength {
                expected: K * 2,
                got: hex_str.len(),
            });
        }

        match hex::decode(hex_str) {
            Ok(decoded) => {
                if decoded.len() != K {
                    return Err(KeyError::InvalidHexLength(decoded.len()));
                }
                let mut value = [0u8; K];
                value.copy_from_slice(&decoded);
                Ok(Key { value })
            }
            Err(e) => Err(KeyError::DecodeHexError(e)),
        }
    }

    ///
    /// Creates a `Key` by taking the SHA-1 hash of the given `input`.
    ///
    /// Useful for generating a key from arbitrary data, e.g., a GUID or filename.
    ///
    pub fn from_input(input: &[u8]) -> Self {
        let mut hasher = Sha1::new();
        hasher.update(input);
        let result = hasher.finalize();
        let mut value: KeyValue = [0u8; K];
        value.copy_from_slice(&result[..K]);
        Key { value }
    }

    ///
    /// Formats the key as a hexadecimal string.
    ///
    pub fn to_hex_string(&self) -> String {
        format!("{:x}", self)
    }

    ///
    /// Returns the XOR distance between `self` and `other` as a byte array.
    ///
    /// This is the main distance calculator distances in the key space.
    ///
    pub fn distance(&self, other: &Key) -> KeyValue {
        let mut distance: KeyValue = [0u8; K];
        for (i, dist) in distance.iter_mut().enumerate() {
            *dist = self.value[i] ^ other.value[i];
        }
        distance
    }

    ///
    /// Count the number of leading zeros in the XOR distance
    ///
    /// Helper for distance calculations in the routing table.
    ///
    pub fn leading_zeros_in_distance(&self, other: &Key) -> usize {
        let distance = self.distance(other);
        let mut count = 0;
        for byte in &distance {
            if *byte == 0 {
                // A zero byte has 8 leading zero bits
                count += 8;
            } else {
                // Partial byte's leading zeros
                count += byte.leading_zeros() as usize;
                break; // Once we hit the first non-zero byte, stop
            }
        }
        count
    }

    ///
    /// For testing purposes in Routing Table
    ///
    pub fn make_exactly_n_same_leading_bits_as(&mut self, key: &Key, mut n_bits: usize) {
        // Copy first n bits from
        let mut byte_i: usize = 0;
        for _ in 0..K {
            if n_bits >= 8 {
                self.value[byte_i] = key.value[byte_i];
                n_bits -= 8;
                byte_i += 1;
            } else {
                let my_byte = &mut self.value[byte_i];
                let your_byte = key.value[byte_i];
                let mut mask: u8 = 0;
                for bit_i in 0..n_bits {
                    mask |= 1u8 << (7 - bit_i);
                }
                *my_byte = mask & your_byte | *my_byte & !mask;
                assert!(n_bits < 8);
            }
        }

        // To make sure exactly same num leading bits, not more, need to set next bit to inverse of target
        if byte_i < K {
            let offset_in_byte: usize = n_bits; // byte_i already incremented!
            assert!(offset_in_byte <= 7);

            // reverse shift because counting from left
            let mask = 1u8 << (7 - offset_in_byte);
            let your_byte = key.value[byte_i];
            let your_bit_neg = mask & !your_byte;
            self.value[byte_i] = your_bit_neg | self.value[byte_i] & !mask;
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.value {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

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

///
/// Implementing LowerHex for Key allows us to use the `{:x}` format specifier, which outputs
/// the key as a lowercase hexadecimal string.
///
impl LowerHex for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in &self.value {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Key {
    ///
    /// Custom deserializer for `Key` that reads a hex string and constructs a `Key`.
    ///
    /// This is used for deserializing keys from configuration files or human user inputs.
    ///
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_str = String::deserialize(deserializer)?;
        Key::from_hex_str(&hex_str).map_err(serde::de::Error::custom)
    }
}

impl Serialize for Key {
    ///
    /// Custom serializer for `Key` that writes the key as a hex string.
    ///
    /// This defines the look of the Key to the human users, we use hex strings for this.
    ///
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let hex_string = self.to_hex_string();
        serializer.serialize_str(&hex_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_keys_are_not_all_zero() {
        let key = Key::new_random();
        assert_ne!(key.value, [0u8; K], "Random key should not be zero array");
    }

    #[test]
    fn test_from_hex_str_ok() {
        let hex_len = K * 2; // Calculate the expected length dynamically
        let valid_hex = "f".repeat(hex_len); // Create a valid hex string dynamically

        let key = Key::from_hex_str(&valid_hex).expect("Should parse fine");
        assert_eq!(key.value.to_vec().len(), K);
        // Ensure it's all 0xFF
        assert!(key.value.to_vec().iter().all(|&b| b == 0xff));
    }

    #[test]
    fn test_from_hex_str_error_wrong_length() {
        let hex_len = K * 2; // Expected length
        let invalid_hex = "f".repeat(hex_len - 1); // Make the length invalid by one less

        let err = Key::from_hex_str(&invalid_hex).unwrap_err();
        match err {
            KeyError::WrongHexStringLength { expected, got } => {
                assert_eq!(expected, hex_len);
                assert_eq!(got, hex_len - 1);
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_from_hex_str_error_decode() {
        let hex_len = K * 2; // Expected length
        let invalid_hex = "z".repeat(hex_len); // Invalid hex characters

        let err = Key::from_hex_str(&invalid_hex).unwrap_err();
        match err {
            KeyError::DecodeHexError(_) => { /* pass */ }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_from_input() {
        use std::fmt::Write;
        let input = b"some data";
        let key = Key::from_input(input);

        let mut hasher = Sha1::new();
        hasher.update(input);
        let hash_result = hasher.finalize();

        let expected_hex =
            hash_result
                .iter()
                .take(K)
                .fold(String::with_capacity(K * 2), |mut acc, &b| {
                    write!(acc, "{:02x}", b).unwrap();
                    acc
                });

        assert_eq!(format!("{}", key), expected_hex);
    }

    #[test]
    fn test_distance_and_leading_zeros() {
        let zero_hex = "0".repeat(K * 2);
        let max_hex = "f".repeat(K * 2);

        let k1 = Key::from_hex_str(&zero_hex).unwrap();
        let k2 = Key::from_hex_str(&max_hex).unwrap();

        let dist = k1.distance(&k2);

        // Ensure all bytes of distance are 0xFF
        assert!(
            dist.iter().all(|&b| b == 0xff),
            "All bytes in the distance should be 0xFF"
        );

        // Calculate the number of leading zeros
        let lz = k1.leading_zeros_in_distance(&k2);
        assert_eq!(lz, 0, "Distance of all 0xFF bytes has 0 leading zeros");
    }

    #[test]
    fn test_ordering() {
        let zero_hex = "0".repeat(K * 2);
        let max_hex = "f".repeat(K * 2);

        let smaller = Key::from_hex_str(&zero_hex).unwrap();
        let bigger = Key::from_hex_str(&max_hex).unwrap();

        assert!(smaller < bigger);
        assert!(bigger > smaller);
        assert_eq!(smaller, smaller);
    }
}
