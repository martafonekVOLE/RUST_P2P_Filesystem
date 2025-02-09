use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Encryption error: {0}")]
    Encryption(String),
}

pub type Result<T> = std::result::Result<T, EncryptionError>;

///
/// Auth tag appended at the end of encrypted payload. Ensures integrity and authenticity.
///
pub const AES_GCM_AUTH_TAG_SIZE_B: usize = 16;

///
/// Encrypts data using AES-GCM.
///
pub fn encrypt_payload(payload: &[u8], key: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

    // Encrypted message will be 16 bytes longer because of auth tag!
    let encrypted_payload = cipher
        .encrypt(&nonce, payload)
        .map_err(|e| EncryptionError::Encryption(e.to_string()))?;

    Ok((nonce.to_vec(), encrypted_payload.to_vec())) // Store nonce + ciphertext
}

///
/// Decrypts data using AES-GCM.
///
pub fn decrypt_payload(encrypted_payload: &Vec<u8>, key: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Nonce::from_slice(nonce);
    // Decrypted message will be 16 bytes shorter
    let decrypted_payload = cipher
        .decrypt(nonce, encrypted_payload.as_slice())
        .map_err(|e| EncryptionError::Encryption(e.to_string()))?;

    Ok(decrypted_payload)
}

pub fn generate_key() -> Vec<u8> {
    Aes256Gcm::generate_key(OsRng).to_vec()
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    pub const AES_GCM_NONCE_SIZE_B: usize = 12;

    #[test]
    fn test_key_generation() {
        let key = generate_key();
        assert_eq!(key.len(), 32); // AES-256 key length
    }

    #[test]
    fn test_encryption_decryption() {
        let key = generate_key();
        let payload = b"Test payload";

        let (nonce, ciphertext) = encrypt_payload(payload, &key).expect("Encryption failed");
        assert_eq!(ciphertext.len(), payload.len() + AES_GCM_AUTH_TAG_SIZE_B);
        assert_ne!(ciphertext, payload); // Ensure payload is encrypted

        let decrypted_payload =
            decrypt_payload(&ciphertext, &key, &nonce).expect("Decryption failed");
        assert_eq!(decrypted_payload, payload); // Ensure payload is decrypted correctly
    }

    #[test]
    fn test_decryption_with_wrong_key() {
        let key = generate_key();
        let wrong_key = generate_key();
        let payload = b"Test payload";

        let (nonce, ciphertext) = encrypt_payload(payload, &key).expect("Encryption failed");

        // Attempt to decrypt with a wrong key
        let result = decrypt_payload(&ciphertext, &wrong_key, &nonce);
        assert!(result.is_err());
        // Attempt to decrypt with a wrong nonce
        let mut rng = rand::thread_rng();
        let index = rng.gen_index(0..AES_GCM_NONCE_SIZE_B);
        let mut modified_nonce = nonce.to_vec();
        modified_nonce[index] ^= rng.random::<u8>();
        let result = decrypt_payload(&ciphertext, &wrong_key, &modified_nonce);
        assert!(result.is_err());
    }

    #[test]
    fn test_decryption_with_modified_ciphertext() {
        let key = generate_key();
        let payload = b"Test payload";
        let payload_size = payload.len();

        let (nonce, mut ciphertext) = encrypt_payload(payload, &key).expect("Encryption failed");
        let mut rng = rand::thread_rng();
        let index = rng.gen_index(0..payload_size);

        ciphertext[index] ^= rng.random::<u8>(); // Modify random byte in ciphertext

        // Attempt to decrypt with modified ciphertext
        let result = decrypt_payload(&ciphertext, &key, &nonce);
        assert!(result.is_err());
    }
}
