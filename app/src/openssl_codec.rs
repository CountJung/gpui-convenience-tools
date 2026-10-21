//! Bounded, in-process PCF/OpenSSL `enc -aes-256-cbc -a` compatibility codec.
//!
//! Uses PKCS#7 and legacy EVP_BytesToKey (one iteration), not PBKDF2. Encryption
//! uses SHA256; decryption tries SHA256, then MD5. This legacy format has **no
//! authentication**: padding/UTF-8 checks cannot reliably detect a wrong key or
//! tampering. Do not use it as a new secure storage or transport protocol.
//! No input, output, or passphrase is logged; errors contain no supplied values.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use md5::Md5;
use sha2::{Digest, Sha256};

/// Maximum UTF-8 byte length of plaintext (before optional LF) and passphrase.
/// Base64 input has a separate 64 KiB bound, including whitespace, so every
/// accepted plaintext can round-trip even with OpenSSL's 64-column wrapping.
pub const MAX_INPUT_BYTES: usize = 32 * 1024;

const MAX_BASE64_INPUT_BYTES: usize = 64 * 1024;
const HEADER: &[u8; 8] = b"Salted__";
const BLOCK_BYTES: usize = 16;
// Include the optional LF and PKCS#7's full extra block at exact boundaries.
const MAX_CIPHER_BYTES: usize = ((MAX_INPUT_BYTES + 1) / BLOCK_BYTES + 1) * BLOCK_BYTES;
const DECRYPT_ERROR: &str = "OpenSSL decryption failed: invalid passphrase, padding, or UTF-8";

/// Encrypts UTF-8 text with a fresh OS-random eight-byte salt and SHA256 KDF.
///
/// The caller controls `append_newline`; pass `true` for PCF's default/`echo`
/// behavior (append exactly one LF, even if the text already ends in a newline).
/// Returns unwrapped Base64 of `Salted__ || salt || AES-256-CBC ciphertext`.
/// Text and passphrase must each be at most [`MAX_INPUT_BYTES`] UTF-8 bytes.
/// This legacy format provides no authentication (see the module warning).
pub fn encrypt(text: &str, passphrase: &str, append_newline: bool) -> Result<String> {
    check_text_limit(text, "Plaintext")?;
    check_text_limit(passphrase, "Passphrase")?;

    let mut salt = [0; 8];
    getrandom::getrandom(&mut salt)
        .map_err(|_| anyhow::anyhow!("Could not obtain a secure OpenSSL salt"))?;
    let mut plaintext = Vec::with_capacity(text.len() + usize::from(append_newline));
    plaintext.extend_from_slice(text.as_bytes());
    if append_newline {
        plaintext.push(b'\n');
    }
    Ok(encrypt_payload::<Sha256>(
        &plaintext,
        passphrase.as_bytes(),
        &salt,
    ))
}

/// Decrypts PCF/OpenSSL Base64, returning text and the digest (`"SHA256"`/`"MD5"`).
///
/// Removes Unicode whitespace from Base64 (including OpenSSL line wrapping),
/// requires the salted header, complete AES blocks, valid PKCS#7 and strict UTF-8,
/// then removes **only trailing CR and LF** like PCF. Leading/internal whitespace,
/// spaces, tabs and embedded NUL are preserved. Tries MD5 if SHA256 fails either
/// padding or UTF-8 validation. No lossy UTF-8 conversion is performed.
///
/// Raw Base64 input is capped at 64 KiB, passphrase at [`MAX_INPUT_BYTES`], and
/// decoded plaintext at that limit plus the optional LF. Empty text/passphrases
/// are allowed for compatibility. This format is unauthenticated: even successful
/// decryption cannot prove the passphrase or ciphertext was correct.
pub fn decrypt(base64: &str, passphrase: &str) -> Result<(String, &'static str)> {
    ensure!(
        base64.len() <= MAX_BASE64_INPUT_BYTES,
        "Base64 input exceeds the 64 KiB limit"
    );
    check_text_limit(passphrase, "Passphrase")?;
    let compact: String = base64.chars().filter(|c| !c.is_whitespace()).collect();
    let payload = STANDARD
        .decode(compact)
        .map_err(|_| anyhow::anyhow!("Invalid OpenSSL Base64 input"))?;
    ensure!(
        payload.len() >= 2 * BLOCK_BYTES && payload.starts_with(HEADER),
        "Invalid OpenSSL Salted__ header or missing ciphertext"
    );
    let cipher = &payload[BLOCK_BYTES..];
    ensure!(
        cipher.len() % BLOCK_BYTES == 0 && cipher.len() <= MAX_CIPHER_BYTES,
        "Invalid OpenSSL ciphertext block length or size limit"
    );
    let mut salt = [0; 8];
    salt.copy_from_slice(&payload[8..BLOCK_BYTES]);

    if let Ok(plaintext) = decrypt_payload::<Sha256>(cipher, passphrase.as_bytes(), &salt) {
        return Ok((plaintext, "SHA256"));
    }
    let plaintext = decrypt_payload::<Md5>(cipher, passphrase.as_bytes(), &salt)?;
    Ok((plaintext, "MD5"))
}

fn check_text_limit(text: &str, label: &str) -> Result<()> {
    ensure!(
        text.len() <= MAX_INPUT_BYTES,
        "{label} exceeds the 32 KiB UTF-8 limit"
    );
    Ok(())
}

fn encrypt_payload<D: Digest>(plaintext: &[u8], passphrase: &[u8], salt: &[u8; 8]) -> String {
    let (key, iv) = derive_key_iv::<D>(passphrase, salt);
    let cipher = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
    let mut payload = Vec::with_capacity(BLOCK_BYTES + cipher.len());
    payload.extend_from_slice(HEADER);
    payload.extend_from_slice(salt);
    payload.extend_from_slice(&cipher);
    STANDARD.encode(payload)
}

fn decrypt_payload<D: Digest>(cipher: &[u8], passphrase: &[u8], salt: &[u8; 8]) -> Result<String> {
    let (key, iv) = derive_key_iv::<D>(passphrase, salt);
    let plaintext = cbc::Decryptor::<aes::Aes256>::new(&key.into(), &iv.into())
        .decrypt_padded_vec_mut::<Pkcs7>(cipher)
        .map_err(|_| anyhow::anyhow!(DECRYPT_ERROR))?;
    // Discard FromUtf8Error: its Debug representation contains decrypted bytes.
    let mut plaintext = String::from_utf8(plaintext).map_err(|_| anyhow::anyhow!(DECRYPT_ERROR))?;
    // Check the size before trimming so CR/LF cannot bypass the bound.
    ensure!(plaintext.len() <= MAX_INPUT_BYTES + 1, DECRYPT_ERROR);
    let trimmed_len = plaintext.trim_end_matches(['\r', '\n']).len();
    plaintext.truncate(trimmed_len);
    ensure!(plaintext.len() <= MAX_INPUT_BYTES, DECRYPT_ERROR);
    Ok(plaintext)
}

/// EVP_BytesToKey: D_i = HASH(D_(i-1) || passphrase || salt), iteration count 1.
fn derive_key_iv<D: Digest>(passphrase: &[u8], salt: &[u8; 8]) -> ([u8; 32], [u8; 16]) {
    let mut material = [0; 48];
    let mut previous = Vec::new();
    let mut filled = 0;
    while filled < material.len() {
        let mut digest = D::new();
        digest.update(&previous);
        digest.update(passphrase);
        digest.update(salt);
        previous = digest.finalize().to_vec();
        let count = previous.len().min(material.len() - filled);
        material[filled..filled + count].copy_from_slice(&previous[..count]);
        filled += count;
    }
    let mut key = [0; 32];
    let mut iv = [0; 16];
    key.copy_from_slice(&material[..32]);
    iv.copy_from_slice(&material[32..]);
    (key, iv)
}

#[cfg(test)]
#[path = "openssl_codec/tests.rs"]
mod tests;
