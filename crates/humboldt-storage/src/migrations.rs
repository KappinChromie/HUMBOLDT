use humboldt_core::{HumboldtError, Result};
use sqlx::SqlitePool;
use tracing::info;

/// Runs all database schema migrations per HUMBOLDT Master Specification.
pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    info!("Executing SQLite database schema migrations...");

    // 1. Observations Telemetry Table (Section 5.2)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS observations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp INTEGER NOT NULL,          -- Unix epoch milliseconds (UTC)
            layer TEXT NOT NULL,                 -- 'aircraft', 'ship', 'satellite', 'quake', 'weather', etc.
            provider TEXT NOT NULL,              -- Slug: 'opensky', 'usgs', 'noaa_swpc', etc.
            entity_id TEXT,                      -- Normalized ID: ICAO24, MMSI, NORAD-ID, Station ID
            latitude REAL,                       -- WGS84 Latitude (-90.0 to 90.0)
            longitude REAL,                      -- WGS84 Longitude (-180.0 to 180.0)
            altitude_m REAL,                     -- Altitude / Elevation in meters
            metric_name TEXT,                    -- e.g., 'kp_index', 'temp_c', 'gauge_height_m'
            metric_value REAL,                   -- Numeric scalar for charting
            attributes TEXT,                     -- Serialized JSON payload of layer-specific attributes
            is_interpolated INTEGER NOT NULL DEFAULT 0 -- 1 if dead-reckoned / estimated, else 0
        );

        CREATE INDEX IF NOT EXISTS idx_obs_layer_time ON observations (layer, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_obs_entity_time ON observations (entity_id, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_obs_metric_time ON observations (metric_name, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_obs_geo ON observations (latitude, longitude) WHERE latitude IS NOT NULL;
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| HumboldtError::Storage(format!("Migration failed for observations: {}", e)))?;

    // 2. Snapshots Table (Section 5.3)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS snapshots (
            id TEXT PRIMARY KEY,                 -- UUIDv4 identifier
            name TEXT NOT NULL,                  -- User-defined label (e.g., 'KC Temp Oct 2026')
            created_at INTEGER NOT NULL,         -- Unix epoch timestamp
            layer TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            query_entity TEXT,                   -- Target location name, boundary ID, or station
            start_time INTEGER NOT NULL,         -- Epoch start of snapshot range
            end_time INTEGER NOT NULL,           -- Epoch end of snapshot range
            resolution TEXT NOT NULL,            -- 'raw', '1h_rollup', '1d_rollup'
            metadata_json TEXT NOT NULL,         -- Serialized provider provenance, unit, coordinates
            data_points BLOB NOT NULL            -- Gzipped, serialized array of (timestamp, value) tuples
        );
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| HumboldtError::Storage(format!("Migration failed for snapshots: {}", e)))?;

    // 3. Vault Items Table (Section 4.1)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS vault_items (
            key_name TEXT PRIMARY KEY,
            encrypted_value BLOB NOT NULL,
            nonce BLOB NOT NULL,
            updated_at INTEGER NOT NULL
        );
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| HumboldtError::Storage(format!("Migration failed for vault_items: {}", e)))?;

    // 4. Hourly Metric Rollups Table (Section 5.4)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS metric_rollups_hourly (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp_hour INTEGER NOT NULL,     -- Unix epoch millis floored to the hour
            layer TEXT NOT NULL,
            provider TEXT NOT NULL,
            entity_id TEXT,
            metric_name TEXT NOT NULL,
            avg_value REAL NOT NULL,
            min_value REAL NOT NULL,
            max_value REAL NOT NULL,
            sample_count INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_rollups_hour ON metric_rollups_hourly (layer, metric_name, timestamp_hour DESC);
        CREATE INDEX IF NOT EXISTS idx_rollups_entity ON metric_rollups_hourly (entity_id, metric_name, timestamp_hour DESC);
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| HumboldtError::Storage(format!("Migration failed for metric_rollups_hourly: {}", e)))?;

    info!("Database schema migrations completed successfully.");
    Ok(())
}
