use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use humboldt_core::{HumboldtError, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPayload {
    pub session_id: String,
    pub client_id: String,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Default)]
struct RateLimitEntry {
    attempts: u32,
    locked_until_epoch_secs: i64,
}

/// Manages signed, encrypted session cookies and anti-brute-force rate limiting for LAN mode.
#[derive(Clone)]
pub struct SessionManager {
    master_key: [u8; 32],
    rate_limits: Arc<Mutex<HashMap<String, RateLimitEntry>>>,
}

impl SessionManager {
    pub fn new(master_key: [u8; 32]) -> Self {
        Self {
            master_key,
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn now_secs() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    }

    /// Creates an encrypted, signed session token for a client.
    /// Expiration defaults to 24 hours (86,400 seconds).
    pub fn create_session_token(&self, client_id: &str, duration_secs: i64) -> Result<String> {
        let now = Self::now_secs();
        let payload = SessionPayload {
            session_id: uuid::Uuid::new_v4().to_string(),
            client_id: client_id.to_string(),
            created_at: now,
            expires_at: now + duration_secs,
        };

        let json_bytes = serde_json::to_vec(&payload)
            .map_err(HumboldtError::Serialization)?;

        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| HumboldtError::Security(format!("Cipher init error: {}", e)))?;

        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, json_bytes.as_ref())
            .map_err(|e| HumboldtError::Security(format!("Session encrypt error: {}", e)))?;

        let token = format!("{}.{}", to_hex(&nonce_bytes), to_hex(&ciphertext));
        Ok(token)
    }

    /// Formats the `Set-Cookie` header string.
    /// CRITICAL REQUIREMENT: Omits the `Secure` flag because LAN mode runs over plain HTTP.
    pub fn create_session_cookie_header(&self, token: &str) -> String {
        format!("humboldt_session={}; HttpOnly; SameSite=Strict; Path=/", token)
    }

    /// Formats the `Set-Cookie` header to revoke and clear the session cookie.
    pub fn create_logout_cookie_header(&self) -> String {
        "humboldt_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0".to_string()
    }

    /// Validates an encrypted session token string, checking authenticity and expiration.
    pub fn validate_session_token(&self, token: &str) -> Option<SessionPayload> {
        let mut parts = token.split('.');
        let nonce_hex = parts.next()?;
        let cipher_hex = parts.next()?;

        let nonce_bytes = from_hex(nonce_hex)?;
        if nonce_bytes.len() != 12 {
            return None;
        }
        let ciphertext = from_hex(cipher_hex)?;

        let cipher = Aes256Gcm::new_from_slice(&self.master_key).ok()?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref()).ok()?;
        let payload: SessionPayload = serde_json::from_slice(&plaintext).ok()?;

        let now = Self::now_secs();
        if payload.expires_at > now {
            Some(payload)
        } else {
            None
        }
    }

    /// Extracts and validates the `humboldt_session` cookie value from a raw Cookie header.
    pub fn validate_cookie_header(&self, cookie_header: &str) -> Option<SessionPayload> {
        for cookie in cookie_header.split(';') {
            let trimmed = cookie.trim();
            if let Some(token) = trimmed.strip_prefix("humboldt_session=") {
                return self.validate_session_token(token);
            }
        }
        None
    }

    /// Checks if a client IP is currently rate-limited due to failed authentication attempts.
    /// Returns Ok(()) if allowed, or Err(remaining_lockout_seconds).
    pub fn check_rate_limit(&self, client_ip: &str) -> std::result::Result<(), u64> {
        let now = Self::now_secs();
        let guard = self.rate_limits.lock().unwrap();
        if let Some(entry) = guard.get(client_ip) {
            if entry.locked_until_epoch_secs > now {
                let remaining = (entry.locked_until_epoch_secs - now) as u64;
                return Err(remaining);
            }
        }
        Ok(())
    }

    /// Records a failed authentication attempt with escalating exponential backoff:
    /// 1st fail: 1s, 2nd fail: 2s, 3rd fail: 4s, 4th fail: 8s, up to 60s max lockout.
    /// Returns the lockout duration applied in seconds.
    pub fn record_failed_attempt(&self, client_ip: &str) -> u64 {
        let now = Self::now_secs();
        let mut guard = self.rate_limits.lock().unwrap();
        let entry = guard.entry(client_ip.to_string()).or_default();
        entry.attempts = entry.attempts.saturating_add(1);

        // Exponential backoff: 2^(attempts - 1), capped at 60s
        let exponent = entry.attempts.saturating_sub(1).min(6);
        let lockout_secs = (1u64 << exponent).min(60);

        entry.locked_until_epoch_secs = now + (lockout_secs as i64);
        lockout_secs
    }

    /// Clears any rate-limiting lockouts upon successful authentication.
    pub fn record_successful_login(&self, client_ip: &str) {
        let mut guard = self.rate_limits.lock().unwrap();
        guard.remove(client_ip);
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
