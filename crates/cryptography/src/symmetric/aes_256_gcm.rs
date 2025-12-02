use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use base64::{Engine as _, engine::general_purpose};

pub const PREFIX: &str = "aes-256-gcm";

/// Encrypt a UTF-8 string using AES-256-GCM.
/// Returns a base64-encoded string containing both the nonce and ciphertext.
#[allow(dead_code)]
pub fn encrypt_string(key: String, plaintext: String) -> Result<String, String> {
    let mut key_bytes = [0u8; 32];
    let key_slice = key.as_bytes();
    key_bytes[..key_slice.len().min(32)].copy_from_slice(&key_slice[..key_slice.len().min(32)]);

    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| format!("Cipher init: {}", e))?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encrypt error: {}", e))?;

    let mut combined = Vec::with_capacity(12 + ciphertext.len());
    combined.extend_from_slice(&nonce);
    combined.extend_from_slice(&ciphertext);
    Ok(general_purpose::STANDARD.encode(combined))
}

pub fn decrypt_string(key: String, encoded: String) -> Result<String, String> {
    let mut key_bytes = [0u8; 32];
    let key_slice = key.as_bytes();
    key_bytes[..key_slice.len().min(32)].copy_from_slice(&key_slice[..key_slice.len().min(32)]);

    let data = general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("Base64 decode: {}", e))?;
    if data.len() < 12 {
        return Err("Ciphertext too short".into());
    }

    let (nonce_bytes, ciphertext) = data.split_at(12);
    #[allow(deprecated)] // https://github.com/fizyk20/generic-array/issues/158
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher =
        Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| format!("Cipher init: {}", e))?;
    let decrypted = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decrypt error: {}", e))?;
    String::from_utf8(decrypted).map_err(|e| format!("UTF-8 decode: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "njAg9MgN5ePYRkrMfAi4ssXrx8qm5Xzd";

    #[test]
    fn test_encrypt_decrypt() {
        let plaintext = "6nK00Qv9SZnkFrXZFSLjGsf0SpVdMgXJ".to_string();
        let encrypted = encrypt_string(TEST_KEY.to_string().clone(), plaintext.clone())
            .expect("Encryption failed");
        let decrypted =
            decrypt_string(TEST_KEY.to_string(), encrypted.clone()).expect("Decryption failed");

        assert_eq!(plaintext.clone(), decrypted.clone());
    }

    #[test]
    fn test_decrypt_known() {
        let encrypted =
            "xVWijH6gLcbeqoEnfEpoknqWH92u+bmX9wDCF7xd1VCg30gpvDQD9/5Ps7fSnWQQyTO6ZYPhpaTQzGeN"
                .to_string();
        let decrypted =
            decrypt_string(TEST_KEY.to_string(), encrypted.clone()).expect("Decryption failed");

        let expected_plaintext = "0estgwdLlyynHq87yBuBfxgWjskfvMCM".to_string();
        assert_eq!(expected_plaintext, decrypted);
    }
}
