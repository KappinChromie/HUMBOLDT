use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use humboldt_core::{HumboldtError, Result};

/// Argon2id parameters matching Architecture Spec Section 4.2:
/// m = 65536 KiB (64 MB), t = 3 iterations, p = 4 lanes.
fn argon2_instance() -> Result<Argon2<'static>> {
    let params = Params::new(65536, 3, 4, None)
        .map_err(|e| HumboldtError::Security(format!("Invalid Argon2 parameters: {}", e)))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

/// Hashes a password using Argon2id with memory-hard parameters.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2_instance()?;
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| HumboldtError::Security(format!("Argon2 hashing error: {}", e)))?
        .to_string();

    Ok(password_hash)
}

/// Verifies a candidate password against an Argon2id hash.
pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|e| HumboldtError::Security(format!("Invalid password hash format: {}", e)))?;

    let argon2 = argon2_instance()?;
    Ok(argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
