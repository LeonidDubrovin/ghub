use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use std::path::Path;

const NONCE_SIZE: usize = 12;

/// Encrypt a plaintext string and write nonce + ciphertext to `ciphertext_path`.
/// The master key is stored (or reused) at `key_path`.
pub fn encrypt_to_file(
    key_path: &Path,
    ciphertext_path: &Path,
    plaintext: &str,
) -> Result<(), String> {
    let key = get_or_create_master_key(key_path)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to init cipher: {}", e))?;

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut output = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    std::fs::write(ciphertext_path, output).map_err(|e| format!("Failed to write ciphertext: {}", e))?;
    Ok(())
}

/// Decrypt a file created by `encrypt_to_file`. Returns `None` if the file does not exist.
pub fn decrypt_from_file(
    key_path: &Path,
    ciphertext_path: &Path,
) -> Result<Option<String>, String> {
    if !ciphertext_path.exists() {
        return Ok(None);
    }

    let key = get_or_create_master_key(key_path)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("Failed to init cipher: {}", e))?;

    let data = std::fs::read(ciphertext_path).map_err(|e| format!("Failed to read ciphertext: {}", e))?;
    if data.len() < NONCE_SIZE {
        return Ok(None);
    }

    let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
    let ciphertext = &data[NONCE_SIZE..];

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {}", e)).map(Some)
}

fn get_or_create_master_key(key_path: &Path) -> Result<Vec<u8>, String> {
    if key_path.exists() {
        return std::fs::read(key_path).map_err(|e| format!("Failed to read master key: {}", e));
    }

    let mut key = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    std::fs::write(key_path, &key).map_err(|e| format!("Failed to write master key: {}", e))?;
    Ok(key)
}
