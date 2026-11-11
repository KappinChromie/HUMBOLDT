use crate::secret::SecretString;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use humboldt_core::{HumboldtError, Result};
use rand::RngCore;
use sqlx::{Row, SqlitePool};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// AES-256-GCM Credential Vault matching Architecture Spec Section 4.1.
#[derive(Clone)]
pub struct Vault {
    pool: SqlitePool,
    master_key: [u8; 32],
}

impl Vault {
    pub fn new(pool: SqlitePool, master_key: [u8; 32]) -> Self {
        Self { pool, master_key }
    }

    /// Encrypts and persists a secret key in the `vault_items` table.
    pub async fn set_secret(&self, key_name: &str, secret_value: &str) -> Result<()> {
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| HumboldtError::Security(format!("Failed to init AES cipher: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, secret_value.as_bytes())
            .map_err(|e| HumboldtError::Security(format!("Encryption failed: {}", e)))?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            r#"
            INSERT OR REPLACE INTO vault_items (key_name, encrypted_value, nonce, updated_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(key_name)
        .bind(ciphertext)
        .bind(nonce_bytes.to_vec())
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Security(format!("Database error saving secret: {}", e)))?;

        info!("Encrypted credential '{}' secured in vault.", key_name);
        Ok(())
    }

    /// Decrypts a secret from the vault into a zeroizing SecretString in memory.
    pub async fn get_secret(&self, key_name: &str) -> Result<Option<SecretString>> {
        let row_opt = sqlx::query(
            "SELECT encrypted_value, nonce FROM vault_items WHERE key_name = ?",
        )
        .bind(key_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HumboldtError::Security(format!("Database error querying secret: {}", e)))?;

        let row = match row_opt {
            Some(r) => r,
            None => return Ok(None),
        };

        let ciphertext: Vec<u8> = row.get("encrypted_value");
        let nonce_bytes: Vec<u8> = row.get("nonce");

        if nonce_bytes.len() != 12 {
            return Err(HumboldtError::Security("Invalid nonce length".to_string()));
        }

        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| HumboldtError::Security(format!("Failed to init AES cipher: {}", e)))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext_bytes = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| HumboldtError::Security(format!("Decryption failed: {}", e)))?;

        let plaintext = String::from_utf8(plaintext_bytes)
            .map_err(|e| HumboldtError::Security(format!("Decrypted string is not valid UTF-8: {}", e)))?;

        Ok(Some(SecretString::new(plaintext)))
    }

    /// Deletes a credential from the vault.
    pub async fn delete_secret(&self, key_name: &str) -> Result<bool> {
        let rows = sqlx::query("DELETE FROM vault_items WHERE key_name = ?")
            .bind(key_name)
            .execute(&self.pool)
            .await
            .map_err(|e| HumboldtError::Security(format!("Failed to delete secret: {}", e)))?
            .rows_affected();

        Ok(rows > 0)
    }

    /// Returns a list of all key names present in the vault (NEVER returns secret values).
    pub async fn list_key_names(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT key_name FROM vault_items ORDER BY key_name ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| HumboldtError::Security(format!("Failed to list vault keys: {}", e)))?;

        Ok(rows.into_iter().map(|r| r.get("key_name")).collect())
    }
}
