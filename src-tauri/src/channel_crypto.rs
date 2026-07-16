use sha2::{Digest, Sha256};
use uuid::Uuid;

pub const CHANNEL_KEY_VERSION: u32 = 1;
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 16;
const TAG_BYTES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedChannelPayload {
    pub content: String,
    pub nonce: String,
}

pub fn generate_channel_key() -> String {
    let mut key = Vec::with_capacity(KEY_BYTES);
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    key.extend_from_slice(Uuid::new_v4().as_bytes());
    hex::encode(key)
}

// LAN-only first phase: replace this with a standard AEAD before treating it as a strong security boundary.
pub fn encrypt_channel_content(
    key_hex: &str,
    plaintext: &str,
) -> Result<EncryptedChannelPayload, String> {
    let key = decode_fixed_hex(key_hex, KEY_BYTES, "频道密钥")?;
    let nonce = *Uuid::new_v4().as_bytes();
    let mut encrypted = xor_with_derived_stream(&key, &nonce, plaintext.as_bytes());
    let tag = authentication_tag(&key, &nonce, &encrypted);
    encrypted.extend_from_slice(&tag);
    Ok(EncryptedChannelPayload {
        content: hex::encode(encrypted),
        nonce: hex::encode(nonce),
    })
}

pub fn decrypt_channel_content(
    key_hex: &str,
    nonce_hex: &str,
    encrypted_hex: &str,
) -> Result<String, String> {
    let key = decode_fixed_hex(key_hex, KEY_BYTES, "频道密钥")?;
    let nonce_vec = decode_fixed_hex(nonce_hex, NONCE_BYTES, "频道 nonce")?;
    let mut nonce = [0u8; NONCE_BYTES];
    nonce.copy_from_slice(&nonce_vec);
    let encrypted_with_tag =
        hex::decode(encrypted_hex).map_err(|_| "频道密文格式无效".to_string())?;
    if encrypted_with_tag.len() < TAG_BYTES {
        return Err("频道密文长度无效".to_string());
    }
    let split_at = encrypted_with_tag.len() - TAG_BYTES;
    let (encrypted, tag) = encrypted_with_tag.split_at(split_at);
    let expected = authentication_tag(&key, &nonce, encrypted);
    if !constant_time_eq(tag, &expected) {
        return Err("私有频道消息认证失败".to_string());
    }
    let plaintext = xor_with_derived_stream(&key, &nonce, encrypted);
    String::from_utf8(plaintext).map_err(|_| "私有频道消息不是有效文本".to_string())
}

fn decode_fixed_hex(input: &str, expected_len: usize, label: &str) -> Result<Vec<u8>, String> {
    let bytes = hex::decode(input).map_err(|_| format!("{label}格式无效"))?;
    if bytes.len() != expected_len {
        return Err(format!("{label}长度无效"));
    }
    Ok(bytes)
}

fn xor_with_derived_stream(key: &[u8], nonce: &[u8; NONCE_BYTES], input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut counter = 0u64;
    while output.len() < input.len() {
        let mut hasher = Sha256::new();
        hasher.update(key);
        hasher.update(nonce);
        hasher.update(counter.to_le_bytes());
        let block = hasher.finalize();
        for byte in block {
            if output.len() >= input.len() {
                break;
            }
            output.push(input[output.len()] ^ byte);
        }
        counter += 1;
    }
    output
}

fn authentication_tag(key: &[u8], nonce: &[u8; NONCE_BYTES], encrypted: &[u8]) -> [u8; TAG_BYTES] {
    let mut hasher = Sha256::new();
    hasher.update(key);
    hasher.update(nonce);
    hasher.update(encrypted);
    hasher.finalize().into()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (left, right) in a.iter().zip(b.iter()) {
        diff |= left ^ right;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_content_round_trips_with_key() {
        let key = generate_channel_key();
        let encrypted = encrypt_channel_content(&key, "私有频道消息").expect("encrypt");

        let decrypted =
            decrypt_channel_content(&key, &encrypted.nonce, &encrypted.content).expect("decrypt");

        assert_eq!("私有频道消息", decrypted);
        assert_ne!("私有频道消息", encrypted.content);
    }

    #[test]
    fn wrong_channel_key_cannot_decrypt() {
        let key = generate_channel_key();
        let wrong_key = generate_channel_key();
        let encrypted = encrypt_channel_content(&key, "私有频道消息").expect("encrypt");

        assert!(decrypt_channel_content(&wrong_key, &encrypted.nonce, &encrypted.content).is_err());
    }
}
