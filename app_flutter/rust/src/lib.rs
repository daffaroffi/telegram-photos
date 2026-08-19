pub mod api;
pub mod backup;
pub mod telegram;
mod frb_generated;

use std::sync::OnceLock;
use telegram_photos_core::db::Db;
use telegram_photos_core::crypto::VaultState;

/// Global database instance, initialized once at app startup.
pub static CORE_DB: OnceLock<Db> = OnceLock::new();

/// Global vault encryption state (key held in memory when unlocked).
pub static VAULT_STATE: OnceLock<VaultState> = OnceLock::new();

/// Get a reference to the global vault state.
pub fn vault_state() -> &'static VaultState {
    VAULT_STATE.get_or_init(VaultState::default)
}
