use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, AES_256_GCM};
use ring::digest;
use ring::rand::{SecureRandom, SystemRandom};
use base64::{Engine as _, engine::general_purpose::STANDARD};

const APP_KEY_SALT: &[u8] = b"EyeCare-v1.0-APISecretKey-2024";

/// Derive a machine-specific 256-bit encryption key using SHA-256
fn derive_key() -> [u8; 32] {
    let machine_id = get_machine_id();
    let input = format!(
        "{}:{}",
        std::str::from_utf8(APP_KEY_SALT).unwrap_or("EyeCare"),
        machine_id
    );
    let hash = digest::digest(&digest::SHA256, input.as_bytes());
    let mut key = [0u8; 32];
    key.copy_from_slice(hash.as_ref());
    key
}

/// Get a machine-specific identifier for key derivation
fn get_machine_id() -> String {
    #[cfg(target_os = "windows")]
    {
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var("USER").unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(target_os = "linux")]
    {
        std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "unknown".to_string())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown".to_string()
    }
}

/// Encrypt plaintext using AES-256-GCM.
/// Returns base64-encoded: nonce(12B) + ciphertext + tag(16B)
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    if plaintext.is_empty() {
        return Ok(String::new());
    }

    let key_bytes = derive_key();
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|e| format!("Key creation failed: {}", e))?;
    let key = LessSafeKey::new(unbound_key);

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12];
    rng.fill(&mut nonce_bytes)
        .map_err(|e| format!("Nonce generation failed: {}", e))?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.as_bytes().to_vec();
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| "AES-GCM seal failed".to_string())?;

    let mut output = nonce_bytes.to_vec();
    output.extend_from_slice(&in_out);
    Ok(STANDARD.encode(&output))
}

/// Decrypt a base64-encoded AES-256-GCM ciphertext
pub fn decrypt(encoded: &str) -> Result<String, String> {
    if encoded.is_empty() {
        return Ok(String::new());
    }

    let data = STANDARD
        .decode(encoded)
        .map_err(|e| format!("Base64 decode failed: {}", e))?;

    if data.len() < 29 {
        // 12 nonce + 1 min ciphertext + 16 tag
        return Err("Encrypted data too short".to_string());
    }

    let key_bytes = derive_key();
    let unbound_key = UnboundKey::new(&AES_256_GCM, &key_bytes)
        .map_err(|e| format!("Key creation failed: {}", e))?;
    let key = LessSafeKey::new(unbound_key);

    let nonce = Nonce::assume_unique_for_key(
        data[..12]
            .try_into()
            .map_err(|_| "Invalid nonce".to_string())?,
    );

    let mut in_out = data[12..].to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::empty(), &mut in_out)
        .map_err(|_| "Decryption failed (wrong key or corrupted)".to_string())?;

    String::from_utf8(plaintext.to_vec()).map_err(|e| format!("Invalid UTF-8: {}", e))
}

/// Check if a stored value is encrypted (prefixed with "enc:")
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with("enc:")
}

/// Encrypt with "enc:" prefix for disk storage
pub fn encrypt_for_storage(plaintext: &str) -> Result<String, String> {
    let encrypted = encrypt(plaintext)?;
    if encrypted.is_empty() {
        Ok(String::new())
    } else {
        Ok(format!("enc:{}", encrypted))
    }
}

/// Decrypt from disk storage, handling "enc:" prefix and legacy plaintext
pub fn decrypt_from_storage(value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if let Some(encrypted) = value.strip_prefix("enc:") {
        decrypt(encrypted)
    } else {
        // Legacy plaintext API key - will be re-encrypted on next save
        log::info!("Found legacy plaintext API key, will re-encrypt on next save");
        Ok(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let secret = "sk-1234567890abcdef";
        let encrypted = encrypt_for_storage(secret).unwrap();
        assert!(is_encrypted(&encrypted));
        assert!(encrypted.starts_with("enc:"));
        let decrypted = decrypt_from_storage(&encrypted).unwrap();
        assert_eq!(decrypted, secret);
    }

    #[test]
    fn test_empty_string() {
        assert_eq!(encrypt_for_storage("").unwrap(), "");
        assert_eq!(decrypt_from_storage("").unwrap(), "");
    }

    #[test]
    fn test_legacy_plaintext() {
        let plaintext = "sk-legacy-key";
        let result = decrypt_from_storage(plaintext).unwrap();
        assert_eq!(result, plaintext);
    }

    #[test]
    fn test_corrupted_data() {
        let result = decrypt("not-valid-base64!!!");
        assert!(result.is_err());
    }
}
