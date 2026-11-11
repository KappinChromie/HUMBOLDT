use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// In-memory zeroizing container for decrypted secrets per Architecture Spec Section 4.1.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    pub fn new(secret: String) -> Self {
        Self { inner: secret }
    }

    /// Access the underlying plaintext secret in-memory.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s.to_string())
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretString(***REDACTED***)")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***REDACTED***")
    }
}
