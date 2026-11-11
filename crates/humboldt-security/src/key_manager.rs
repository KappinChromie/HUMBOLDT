use humboldt_core::Result;
use rand::RngCore;
use std::fs;
use std::path::Path;
use tracing::info;
use zeroize::Zeroize;

pub const KEYRING_SERVICE: &str = "Humboldt";
pub const KEYRING_USER: &str = "Humboldt_Master_Vault_Key";

/// Resolves or initializes the 256-bit (32-byte) AES master key per Section 4.1.
pub fn resolve_master_key(data_dir: &Path) -> Result<[u8; 32]> {
    // 1. Environment Variable Override
    if let Ok(env_key) = std::env::var("HUMBOLDT_MASTER_KEY") {
        let trimmed = env_key.trim();
        if let Ok(bytes) = hex::decode(trimmed) {
            if bytes.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&bytes);
                info!("Master vault key resolved from HUMBOLDT_MASTER_KEY environment variable.");
                return Ok(key);
            }
        }
    }

    // 2. OS Keyring (Windows Credential Manager / Linux Secret Service)
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        if let Ok(stored_hex) = entry.get_password() {
            if let Ok(bytes) = hex::decode(&stored_hex) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    info!("Master vault key resolved from OS secure credential store.");
                    return Ok(key);
                }
            }
        }

        // Try generating and writing to OS keyring
        let mut key = generate_random_key();
        let hex_key = hex::encode(key);
        if entry.set_password(&hex_key).is_ok() {
            info!("Generated and registered new master vault key in OS credential store.");
            return Ok(key);
        }
        key.zeroize();
    }

    // 3. Fallback: Encrypted Keyfile in data folder (.vault.key)
    let keyfile_path = data_dir.join(".vault.key");
    if keyfile_path.exists() {
        if let Ok(stored_hex) = fs::read_to_string(&keyfile_path) {
            if let Ok(bytes) = hex::decode(stored_hex.trim()) {
                if bytes.len() == 32 {
                    let mut key = [0u8; 32];
                    key.copy_from_slice(&bytes);
                    info!("Master vault key resolved from keyfile {:?}", keyfile_path);
                    return Ok(key);
                }
            }
        }
    }

    // Generate new keyfile
    let key = generate_random_key();
    let hex_key = hex::encode(key);
    fs::create_dir_all(data_dir)?;
    fs::write(&keyfile_path, hex_key)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&keyfile_path, fs::Permissions::from_mode(0o600));
    }

    info!("Generated new master vault keyfile at {:?}", keyfile_path);
    Ok(key)
}

fn generate_random_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);
    key
}

mod hex {
    pub fn encode<T: AsRef<[u8]>>(data: T) -> String {
        data.as_ref().iter().map(|b| format!("{:02x}", b)).collect()
    }

    pub fn decode(hex_str: &str) -> std::result::Result<Vec<u8>, ()> {
        if !hex_str.len().is_multiple_of(2) {
            return Err(());
        }
        (0..hex_str.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).map_err(|_| ()))
            .collect()
    }
}
