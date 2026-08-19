//! FRB bridge for client-side encryption (zero-knowledge).
//!
//! Wraps the core crypto module so Dart can set up, unlock, encrypt, and
//! decrypt the vault without touching Rust internals directly.

use telegram_photos_core::crypto;

use crate::vault_state;
use crate::CORE_DB;

/// Set up encryption vault with a new passphrase.
/// Generates salt, derives key via Argon2id, persists KDF metadata.
/// Returns true on success.
pub fn vault_setup(passphrase: String) -> Result<bool, String> {
    let db = CORE_DB.get().ok_or("DB not initialized")?;
    let state = vault_state();
    crypto::vault_setup(db, &passphrase, state)
}

/// Unlock an existing vault with the passphrase.
/// Re-derives the key from stored salt, holds it in memory.
pub fn vault_unlock(passphrase: String) -> Result<bool, String> {
    let db = CORE_DB.get().ok_or("DB not initialized")?;
    let state = vault_state();
    crypto::vault_unlock(db, &passphrase, state)
}

/// Lock the vault — drops the in-memory key.
pub fn vault_lock() -> Result<bool, String> {
    let state = vault_state();
    crypto::vault_lock(state)
}

/// Returns vault status: enabled, passphraseSet, unlocked.
pub fn vault_status() -> Result<VaultStatus, String> {
    let db = CORE_DB.get().ok_or("DB not initialized")?;
    let state = vault_state();
    let val = crypto::vault_status(db, state)?;
    Ok(VaultStatus {
        enabled: val.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
        passphrase_set: val.get("passphraseSet").and_then(|v| v.as_bool()).unwrap_or(false),
        unlocked: val.get("unlocked").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

/// Encrypt a file (plaintext -> .tdenc envelope).
pub fn encrypt_file(src_path: String, dst_path: String) -> Result<(), String> {
    let state = vault_state();
    let key_bytes = state
        .key
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("Vault is locked — unlock first")?
        .clone();
    let key = crypto::VaultKey(key_bytes.try_into().map_err(|_| "Invalid key length")?);
    crypto::encrypt_file(
        std::path::Path::new(&src_path),
        std::path::Path::new(&dst_path),
        &key,
    )
}

/// Decrypt a file (.tdenc envelope -> plaintext).
pub fn decrypt_file(src_path: String, dst_path: String) -> Result<(), String> {
    let state = vault_state();
    let key_bytes = state
        .key
        .lock()
        .map_err(|e| e.to_string())?
        .as_ref()
        .ok_or("Vault is locked — unlock first")?
        .clone();
    let key = crypto::VaultKey(key_bytes.try_into().map_err(|_| "Invalid key length")?);
    crypto::decrypt_file(
        std::path::Path::new(&src_path),
        std::path::Path::new(&dst_path),
        &key,
    )
}

pub struct VaultStatus {
    pub enabled: bool,
    pub passphrase_set: bool,
    pub unlocked: bool,
}
