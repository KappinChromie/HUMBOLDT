use humboldt_core::{HumboldtError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;
use tracing::info;

/// Creates an embedded SQLite connection pool configured per Architecture Spec Section 5.1.
pub async fn create_sqlite_pool(db_path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn_str = format!("sqlite://{}", db_path.to_string_lossy());
    let options = SqliteConnectOptions::from_str(&conn_str)
        .map_err(|e| HumboldtError::Storage(format!("Invalid SQLite path: {}", e)))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_millis(5000))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(16)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect_with(options)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to connect to SQLite: {}", e)))?;

    // Apply cache size pragma (-64000 = 64 MB)
    sqlx::query("PRAGMA cache_size = -64000;")
        .execute(&pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to set cache_size pragma: {}", e)))?;

    info!("SQLite database pool established at {:?}", db_path);
    Ok(pool)
}

/// Creates an in-memory SQLite connection pool for isolated offline testing.
pub async fn create_in_memory_pool() -> Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| HumboldtError::Storage(format!("Invalid memory SQLite path: {}", e)))?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_millis(5000))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to create memory SQLite: {}", e)))?;

    sqlx::query("PRAGMA cache_size = -64000;")
        .execute(&pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to set cache_size pragma: {}", e)))?;

    Ok(pool)
}
