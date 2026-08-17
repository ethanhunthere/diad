//! Secure credential storage using the system keyring (libsecret/GNOME Keyring).
//!
//! Security contract:
//! - Tokens are stored in the OS keyring (never plaintext on disk).
//! - If no secret service is available (e.g. the live ISO), the vault falls
//!   back to IN-MEMORY-ONLY storage with a visible warning that keys will not
//!   persist. Secrets are never written to disk, never logged, and never
//!   included in session state or LLM context.

use std::collections::HashMap;
use std::sync::Mutex;

use keyring::Entry;

/// Storage backend for credentials.
enum VaultBackend {
    /// OS keyring (libsecret / GNOME Keyring on Linux).
    Keyring,
    /// In-memory fallback used when no secret service is available. Secrets
    /// live only for the lifetime of this process and are lost on exit.
    Memory(Mutex<HashMap<String, String>>),
}

/// Manages secure storage of API tokens.
///
/// On Linux, this uses `libsecret` (GNOME Keyring) when available; otherwise
/// tokens are held in memory only (with a warning).
pub struct Vault {
    /// Service name for keyring entries
    service: String,
    /// Active storage backend (probed at construction).
    backend: VaultBackend,
}

impl Vault {
    /// Create a new Vault instance.
    ///
    /// The service name is used as the keyring entry namespace.
    ///
    /// Probes whether the OS secret service is reachable with a real
    /// round-trip (set + delete of a probe credential — `Entry::new` alone is
    /// lazy and would always succeed). If the probe fails, falls back to
    /// in-memory storage and logs a prominent warning.
    pub fn new() -> anyhow::Result<Self> {
        Self::new_with_service("diad-agent")
    }

    /// Create a new Vault instance with a custom service name namespace.
    pub fn new_with_service(service: impl Into<String>) -> anyhow::Result<Self> {
        let service = service.into();
        let backend = match Self::probe_keyring(&service) {
            true => {
                tracing::info!("Vault: using OS keyring ({service})");
                VaultBackend::Keyring
            }
            false => {
                // Visible warning: credentials will not survive this session.
                tracing::warn!(
                    "Vault: no OS secret service available (e.g. running from the \
                     live ISO). API keys will be held in MEMORY ONLY and will \
                     NOT persist after this session. Nothing is written to disk."
                );
                eprintln!(
                    "[diad] WARNING: No OS secret service available — API keys will \
                     be kept in memory only and will NOT persist."
                );
                VaultBackend::Memory(Mutex::new(HashMap::new()))
            }
        };
        Ok(Self { service, backend })
    }

    /// Probe the OS keyring with an actual credential round-trip.
    ///
    /// Returns `true` only when a *second, freshly-created* entry can read
    /// back a credential written by the first. This detects:
    /// - no secret service at all (live ISO) — `set_password` fails;
    /// - keyring's per-entry mock store (compiled without platform features)
    ///   — the fresh entry finds nothing, so no real persistence exists.
    fn probe_keyring(service: &str) -> bool {
        const PROBE: &str = "__diad_vault_probe__";
        let name = format!("{service}-probe");
        let entry = match Entry::new(&name, service) {
            Ok(e) => e,
            Err(_) => return false,
        };
        if entry.set_password(PROBE).is_err() {
            return false;
        }
        let verify = match Entry::new(&name, service) {
            Ok(e) => e,
            Err(_) => {
                let _ = entry.delete_credential();
                return false;
            }
        };
        let ok = matches!(verify.get_password(), Ok(p) if p == PROBE);
        let _ = entry.delete_credential();
        ok
    }

    fn keyring_entry(&self, provider: &str) -> Entry {
        let entry_name = format!("{}-{}", self.service, provider);
        Entry::new(&entry_name, &self.service)
            .expect("keyring Entry::new cannot fail after successful probe")
    }

    /// Store a token for a provider.
    ///
    /// The token is stored in the system keyring under
    /// the entry name `diad-agent-{provider}` — or in memory only when the
    /// keyring is unavailable. Tokens are never logged.
    pub fn store_token(&self, provider: &str, token: &str) -> anyhow::Result<()> {
        match &self.backend {
            VaultBackend::Keyring => {
                self.keyring_entry(provider).set_password(token)?;
            }
            VaultBackend::Memory(store) => {
                store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("vault memory store poisoned"))?
                    .insert(provider.to_string(), token.to_string());
            }
        }
        tracing::info!("Token stored for provider: {provider}");
        Ok(())
    }

    /// Retrieve a token for a provider.
    ///
    /// Returns `None` if no token is stored for the given provider.
    pub fn get_token(&self, provider: &str) -> Option<String> {
        match &self.backend {
            VaultBackend::Keyring => match self.keyring_entry(provider).get_password() {
                Ok(token) => {
                    tracing::debug!("Token retrieved for provider: {provider}");
                    Some(token)
                }
                Err(e) => {
                    tracing::debug!("No token found for {provider}: {e}");
                    None
                }
            },
            VaultBackend::Memory(store) => match store.lock() {
                Ok(guard) => guard.get(provider).cloned(),
                Err(_) => None,
            },
        }
    }

    /// Delete a stored token for a provider.
    pub fn delete_token(&self, provider: &str) -> anyhow::Result<()> {
        match &self.backend {
            VaultBackend::Keyring => {
                match self.keyring_entry(provider).delete_credential() {
                    Ok(()) => {
                        tracing::info!("Token deleted for provider: {provider}");
                        Ok(())
                    }
                    Err(e) => {
                        // Ignore "not found" errors
                        tracing::debug!("No token to delete for {provider}: {e}");
                        Ok(())
                    }
                }
            }
            VaultBackend::Memory(store) => {
                store
                    .lock()
                    .map_err(|_| anyhow::anyhow!("vault memory store poisoned"))?
                    .remove(provider);
                tracing::info!("Token deleted for provider: {provider}");
                Ok(())
            }
        }
    }

    /// List all providers that have stored tokens.
    pub fn list_connected_providers(&self) -> Vec<String> {
        let providers = vec!["anthropic", "gemini", "openai", "qwen"];
        providers
            .into_iter()
            .filter(|p| self.get_token(p).is_some())
            .map(|s| s.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both tests touch the real secret service over D-Bus; serialize them
    // and use unique service namespaces so parallel test threads cannot race.
    static KEYRING_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_vault_roundtrip() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let service = format!("diad-test-{}", uuid::Uuid::new_v4());
        let vault =
            Vault::new_with_service(&service).expect("vault must construct even without a keyring");
        vault.store_token("gemini", "sk-test").unwrap();
        assert_eq!(vault.get_token("gemini").as_deref(), Some("sk-test"));
        assert!(vault
            .list_connected_providers()
            .contains(&"gemini".to_string()));
        vault.delete_token("gemini").unwrap();
        assert_eq!(vault.get_token("gemini"), None);
    }

    #[test]
    fn test_vault_new_never_fails() {
        let _guard = KEYRING_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let service = format!("diad-test-{}", uuid::Uuid::new_v4());
        let _ = Vault::new_with_service(&service)
            .expect("Vault::new must return Ok even with no secret service");
    }
}
