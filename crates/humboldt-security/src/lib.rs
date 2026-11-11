pub mod auth;
pub mod key_manager;
pub mod secret;
pub mod session;
pub mod vault;

pub use auth::{hash_password, verify_password};
pub use key_manager::resolve_master_key;
pub use secret::SecretString;
pub use session::{SessionManager, SessionPayload};
pub use vault::Vault;

#[cfg(test)]
mod tests {
    use super::*;
    use humboldt_storage::Storage;

    #[tokio::test]
    async fn test_vault_encryption_and_decryption() {
        let storage = Storage::in_memory().await.unwrap();
        let master_key = [7u8; 32];
        let vault = Vault::new(storage.pool.clone(), master_key);

        vault
            .set_secret("OPENWEATHER_API_KEY", "test_secret_abc123")
            .await
            .expect("Failed to set secret");

        // Decrypt
        let retrieved = vault
            .get_secret("OPENWEATHER_API_KEY")
            .await
            .expect("Failed to get secret")
            .expect("Secret should exist");

        assert_eq!(retrieved.expose_secret(), "test_secret_abc123");

        // Verify keys list
        let keys = vault.list_key_names().await.expect("Failed to list keys");
        assert_eq!(keys, vec!["OPENWEATHER_API_KEY"]);

        // Delete secret
        let deleted = vault
            .delete_secret("OPENWEATHER_API_KEY")
            .await
            .expect("Failed to delete secret");
        assert!(deleted);

        let recheck = vault
            .get_secret("OPENWEATHER_API_KEY")
            .await
            .expect("Recheck failed");
        assert!(recheck.is_none());
    }

    #[test]
    fn test_argon2id_password_hashing() {
        let pwd = "SuperSecretHumboldtPassword2026!";
        let hash = hash_password(pwd).expect("Hashing failed");
        assert!(hash.starts_with("$argon2id$"));

        let is_valid = verify_password(pwd, &hash).expect("Verification failed");
        assert!(is_valid);

        let is_invalid = verify_password("WrongPassword!", &hash).expect("Verification failed");
        assert!(!is_invalid);
    }

    #[test]
    fn test_lan_session_cookie_lifecycle_and_rate_limiting() {
        let master_key = [15u8; 32];
        let session_mgr = SessionManager::new(master_key);

        // 1. Create session token (valid for 3600 seconds)
        let token = session_mgr
            .create_session_token("192.168.1.50", 3600)
            .expect("Token generation failed");

        // 2. Format cookie header - MUST OMIT Secure flag for plain HTTP LAN mode
        let cookie_header = session_mgr.create_session_cookie_header(&token);
        assert!(cookie_header.starts_with("humboldt_session="));
        assert!(cookie_header.contains("HttpOnly"));
        assert!(cookie_header.contains("SameSite=Strict"));
        assert!(cookie_header.contains("Path=/"));
        assert!(
            !cookie_header.contains("Secure"),
            "LAN session cookies must omit Secure flag over plain HTTP"
        );

        // 3. Validate cookie
        let payload = session_mgr
            .validate_cookie_header(&cookie_header)
            .expect("Session validation should succeed");
        assert_eq!(payload.client_id, "192.168.1.50");
        assert!(payload.expires_at > payload.created_at);

        // 4. Test expired token (-10 seconds)
        let expired_token = session_mgr
            .create_session_token("192.168.1.50", -10)
            .expect("Token generation failed");
        assert!(session_mgr.validate_session_token(&expired_token).is_none());

        // 5. Rate limiting test
        let test_ip = "192.168.1.99";
        assert!(session_mgr.check_rate_limit(test_ip).is_ok());

        // 1st fail: 1s lockout
        let lock1 = session_mgr.record_failed_attempt(test_ip);
        assert_eq!(lock1, 1);
        assert!(session_mgr.check_rate_limit(test_ip).is_err());

        // 2nd fail: 2s lockout
        let lock2 = session_mgr.record_failed_attempt(test_ip);
        assert_eq!(lock2, 2);

        // 3rd fail: 4s lockout
        let lock3 = session_mgr.record_failed_attempt(test_ip);
        assert_eq!(lock3, 4);

        // 4th fail: 8s lockout
        let lock4 = session_mgr.record_failed_attempt(test_ip);
        assert_eq!(lock4, 8);

        // Success resets lockout
        session_mgr.record_successful_login(test_ip);
        assert!(session_mgr.check_rate_limit(test_ip).is_ok());
    }
}
