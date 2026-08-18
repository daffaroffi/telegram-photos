//! Zero-Knowledge client-side encryption (PRD section 4.8).
//!
//! Files are encrypted locally with XChaCha20-Poly1305 (streaming, 4 KB
//! chunks with a BE32 counter nonce) before upload to Telegram, using a
//! 32-byte key derived from the user's passphrase with Argon2id. The
//! passphrase is never stored; only the salt and KDF parameters live in the
//! local database, so Telegram cannot read the file contents.

use crate::db::Db;
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::stream::{DecryptorBE32, EncryptorBE32};
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use std::io::{Read, Write};
use std::path::Path;

pub const ENCRYPTED_MAGIC: &[u8; 8] = b"TPENC1v1";

pub struct VaultKey(pub [u8; 32]);

/// KDF parameters (Argon2id). PRD 4.8: key derived from passphrase.
const ARGON_M_COST: u32 = 64 * 1024; // 64 MiB
const ARGON_T_COST: u32 = 3;
const ARGON_P_COST: u32 = 1;

pub fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    let params = Params::new(ARGON_M_COST, ARGON_T_COST, ARGON_P_COST, Some(32))
        .map_err(|e| e.to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0u8; 32];
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| format!("Gagal menurunkan kunci: {}", e))?;
    Ok(key)
}

/// Creates the vault: generates a random salt, derives the key and persists
/// the KDF metadata in the database. Returns the derived key.
pub fn setup_vault(db: &Db, passphrase: &str) -> Result<VaultKey, String> {
    if passphrase.trim().is_empty() {
        return Err("Passphrase tidak boleh kosong.".into());
    }
    let salt: [u8; 16] = rand::random();
    let key = derive_key(passphrase, &salt)?;
    let meta = serde_json::json!({
        "salt": hex::encode(salt),
        "argon_m_cost": ARGON_M_COST,
        "argon_t_cost": ARGON_T_COST,
        "argon_p_cost": ARGON_P_COST,
    });
    db.set_vault_meta(&meta.to_string()).map_err(|e| e.to_string())?;
    Ok(VaultKey(key))
}

/// Re-derives the key from the stored salt. Used to unlock the vault.
pub fn unlock_vault(db: &Db, passphrase: &str) -> Result<VaultKey, String> {
    let meta = db
        .get_vault_meta()
        .map_err(|e| e.to_string())?
        .ok_or("Vault belum diaktifkan.")?;
    let v: serde_json::Value =
        serde_json::from_str(&meta).map_err(|e| format!("Metadata vault korup: {}", e))?;
    let salt_hex = v
        .get("salt")
        .and_then(|s| s.as_str())
        .ok_or("Metadata vault tidak memiliki salt.")?;
    let salt = hex::decode(salt_hex).map_err(|e| e.to_string())?;
    let key = derive_key(passphrase, &salt)?;
    Ok(VaultKey(key))
}

/// In-memory vault key holder. Managed by the caller.
pub struct VaultState {
    pub key: std::sync::Mutex<Option<Vec<u8>>>,
}

impl Default for VaultState {
    fn default() -> Self {
        Self {
            key: std::sync::Mutex::new(None),
        }
    }
}

/// Streams `src` into `dst` as an XChaCha20-Poly1305 encrypted envelope.
pub fn encrypt_file(
    src: &Path,
    dst: &Path,
    key: &VaultKey,
) -> Result<(), String> {
    use chacha20poly1305::aead::Payload;

    // STREAM construction: XChaCha nonce (24 bytes) minus 5 bytes of
    // BE32 counter + last-block flag = 19 bytes of app-level nonce.
    let nonce_bytes: [u8; 19] = rand::random();
    let nonce = generic_array::GenericArray::from_slice(&nonce_bytes);
    let cipher = XChaCha20Poly1305::new_from_slice(&key.0).map_err(|e| e.to_string())?;
    let mut encryptor = EncryptorBE32::from_aead(cipher, nonce);

    let mut input = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut output = std::fs::File::create(dst).map_err(|e| e.to_string())?;
    output.write_all(ENCRYPTED_MAGIC).map_err(|e| e.to_string())?;
    output.write_all(&nonce_bytes).map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; 4 * 1024];
    loop {
        let n = input.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let chunk = encryptor
            .encrypt_next(Payload { msg: &buf[..n], aad: &[] })
            .map_err(|e| format!("Enkripsi gagal: {}", e))?;
        output.write_all(&chunk).map_err(|e| e.to_string())?;
    }
    let last = encryptor
        .encrypt_last(Payload { msg: &[], aad: &[] })
        .map_err(|e| format!("Enkripsi gagal: {}", e))?;
    output.write_all(&last).map_err(|e| e.to_string())?;
    Ok(())
}

/// Streams an encrypted envelope back to plaintext, verifying the MAC.
pub fn decrypt_file(
    src: &Path,
    dst: &Path,
    key: &VaultKey,
) -> Result<(), String> {
    use chacha20poly1305::aead::Payload;

    let mut input = std::fs::File::open(src).map_err(|e| e.to_string())?;
    let mut header = [0u8; 8 + 19];
    input
        .read_exact(&mut header)
        .map_err(|_| "File terenkripsi korup (header pendek).".to_string())?;
    if &header[..8] != ENCRYPTED_MAGIC {
        return Err("File bukan format enkripsi Telegram Photos.".into());
    }
    let nonce = generic_array::GenericArray::from_slice(&header[8..]);
    let cipher = XChaCha20Poly1305::new_from_slice(&key.0).map_err(|e| e.to_string())?;
    let mut decryptor = DecryptorBE32::from_aead(cipher, nonce);

    let total = input.metadata().map_err(|e| e.to_string())?.len();
    let header_len: u64 = 8 + 19;
    let data_len = total.saturating_sub(header_len).saturating_sub(16);

    let mut output = std::fs::File::create(dst).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 4 * 1024 + 16];
    let mut read_so_far: u64 = 0;
    while read_so_far < data_len {
        let want = ((data_len - read_so_far) as usize).min(buf.len());
        let n = input.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("File terenkripsi terpotong.".into());
        }
        read_so_far += n as u64;
        let plain = decryptor
            .decrypt_next(Payload { msg: &buf[..n], aad: &[] })
            .map_err(|e| format!("Verifikasi MAC gagal (passphrase salah?): {}", e))?;
        output.write_all(&plain).map_err(|e| e.to_string())?;
    }

    let mut tail = [0u8; 16];
    input
        .read_exact(&mut tail)
        .map_err(|_| "File terenkripsi terpotong (tag akhir hilang).".to_string())?;
    let last = decryptor
        .decrypt_last(Payload { msg: &tail, aad: &[] })
        .map_err(|e| format!("Verifikasi MAC gagal (passphrase salah?): {}", e))?;
    output.write_all(&last).map_err(|e| e.to_string())?;
    Ok(())
}

/// Setup vault with passphrase and unlock it in memory.
pub fn vault_setup(
    db: &Db,
    passphrase: &str,
    vault_state: &VaultState,
) -> Result<bool, String> {
    let key = setup_vault(db, passphrase)?;
    *vault_state.key.lock().unwrap() = Some(key.0.to_vec());
    let mut settings = db.get_settings().map_err(|e| e.to_string())?;
    settings.client_encryption_enabled = true;
    settings.vault_passphrase_set = true;
    db.save_settings(&settings).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Unlocks the vault with the passphrase (key held only in memory).
pub fn vault_unlock(
    db: &Db,
    passphrase: &str,
    vault_state: &VaultState,
) -> Result<bool, String> {
    let key = unlock_vault(db, passphrase)?;
    *vault_state.key.lock().unwrap() = Some(key.0.to_vec());
    Ok(true)
}

/// Locks the vault, dropping the in-memory key.
pub fn vault_lock(vault_state: &VaultState) -> Result<bool, String> {
    *vault_state.key.lock().unwrap() = None;
    Ok(true)
}

/// Returns vault status as JSON.
pub fn vault_status(
    db: &Db,
    vault_state: &VaultState,
) -> Result<serde_json::Value, String> {
    let settings = db.get_settings().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "enabled": settings.client_encryption_enabled,
        "passphraseSet": settings.vault_passphrase_set,
        "unlocked": vault_state.key.lock().unwrap().is_some(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("photo.jpg");
        let enc = dir.path().join("photo.jpg.tdenc");
        let dec = dir.path().join("photo_dec.jpg");

        let content: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&src, &content).unwrap();

        let key = VaultKey([7u8; 32]);
        encrypt_file(&src, &enc, &key).unwrap();
        decrypt_file(&enc, &dec, &key).unwrap();

        let restored = std::fs::read(&dec).unwrap();
        assert_eq!(restored, content, "decrypted bytes must match original");

        // Wrong key must fail MAC verification.
        let wrong = VaultKey([8u8; 32]);
        assert!(decrypt_file(&enc, &dir.path().join("bad.jpg"), &wrong).is_err());
    }

    #[test]
    fn derive_key_is_deterministic() {
        let salt = [1u8; 16];
        let k1 = derive_key("rahasia123", &salt).unwrap();
        let k2 = derive_key("rahasia123", &salt).unwrap();
        let k3 = derive_key("rahasia124", &salt).unwrap();
        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
    }
}
