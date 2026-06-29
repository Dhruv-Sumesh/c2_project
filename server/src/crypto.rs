use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn derive_key_from_psk(psk: &str) -> [u8; 32] {
    let hash = Sha256::digest(psk.as_bytes());
    hash.into()
}

pub fn key_from_hex(hex_key: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_key).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("session key must be 32 bytes".to_string());
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(key)
}

pub fn encrypt(plaintext: &[u8], key: &[u8; 32]) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| e.to_string())?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend(ciphertext);
    Ok(BASE64.encode(combined))
}

pub fn decrypt(encoded: &str, key: &[u8; 32]) -> Result<Vec<u8>, String> {
    let combined = BASE64.decode(encoded).map_err(|e| e.to_string())?;
    if combined.len() < 12 {
        return Err("ciphertext too short".to_string());
    }

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| e.to_string())?;
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| e.to_string())
}

pub fn generate_session_key_hex() -> String {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    hex::encode(key)
}
