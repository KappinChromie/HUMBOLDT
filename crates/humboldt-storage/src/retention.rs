use humboldt_core::{HumboldtError, Result};
use sqlx::{Row, SqlitePool};
use tracing::{info, warn};

pub const DEFAULT_DISK_CAP_BYTES: u64 = 10 * 1024 * 1024 * 1024; // 10 GB

/// Report of purged and rolled up records from automated retention sweep.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RetentionReport {
    pub ephemeral_purged: u64,
    pub live_tracking_downsampled: u64,
    pub live_tracking_purged: u64,
    pub metric_series_rolled_up: u64,
    pub metric_series_raw_purged: u64,
    pub discrete_events_purged: u64,
    pub disk_cap_triggered: bool,
    pub initial_size_bytes: u64,
    pub final_size_bytes: u64,
}

#[derive(Clone)]
pub struct RetentionManager {
    pool: SqlitePool,
    disk_cap_bytes: u64,
}

impl RetentionManager {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            disk_cap_bytes: DEFAULT_DISK_CAP_BYTES,
        }
    }

    pub fn with_disk_cap(pool: SqlitePool, disk_cap_bytes: u64) -> Self {
        Self {
            pool,
            disk_cap_bytes,
        }
    }

    /// Executes retention pruning sweep according to Architecture Spec Section 5.4.
    pub async fn run_sweep(&self, now_ms: i64) -> Result<RetentionReport> {
        let mut report = RetentionReport {
            initial_size_bytes: self.get_database_size_bytes().await.unwrap_or(0),
            ..Default::default()
        };

        // 1. Ephemeral: Local News, GDELT Articles, Active Alerts (> 2 hours)
        let ephemeral_cutoff = now_ms - (2 * 3600 * 1000);
        let ephem_rows = sqlx::query(
            "DELETE FROM observations WHERE layer IN ('news', 'alert') AND timestamp < ?",
        )
        .bind(ephemeral_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Ephemeral sweep failed: {}", e)))?
        .rows_affected();
        report.ephemeral_purged = ephem_rows;

        // 2. Live Tracking Downsampling (between 2h and 48h old -> downsample to 1 pt/min)
        let tracking_2h_cutoff = now_ms - (2 * 3600 * 1000);
        let tracking_48h_cutoff = now_ms - (48 * 3600 * 1000);

        // Delete duplicates within the same minute for each entity, retaining min(id)
        let downsample_rows = sqlx::query(
            r#"
            DELETE FROM observations
            WHERE layer IN ('aircraft', 'ship', 'satellite')
              AND timestamp >= ? AND timestamp < ?
              AND id NOT IN (
                  SELECT MIN(id)
                  FROM observations
                  WHERE layer IN ('aircraft', 'ship', 'satellite')
                    AND timestamp >= ? AND timestamp < ?
                  GROUP BY entity_id, (timestamp / 60000)
              )
            "#,
        )
        .bind(tracking_48h_cutoff)
        .bind(tracking_2h_cutoff)
        .bind(tracking_48h_cutoff)
        .bind(tracking_2h_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Tracking downsampling failed: {}", e)))?
        .rows_affected();
        report.live_tracking_downsampled = downsample_rows;

        // Purge tracking older than 48 hours completely
        let live_rows = sqlx::query(
            "DELETE FROM observations WHERE layer IN ('aircraft', 'ship', 'satellite') AND timestamp < ?",
        )
        .bind(tracking_48h_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Live tracking 48h sweep failed: {}", e)))?
        .rows_affected();
        report.live_tracking_purged = live_rows;

        // 3. Metric Series: Rollup raw metrics older than 14 days into metric_rollups_hourly
        let metric_14d_cutoff = now_ms - (14 * 86400 * 1000);
        let rolled_up = sqlx::query(
            r#"
            INSERT INTO metric_rollups_hourly (
                timestamp_hour, layer, provider, entity_id, metric_name, avg_value, min_value, max_value, sample_count
            )
            SELECT
                (timestamp / 3600000) * 3600000 AS timestamp_hour,
                layer,
                provider,
                COALESCE(entity_id, 'global') AS entity_id,
                metric_name,
                AVG(metric_value),
                MIN(metric_value),
                MAX(metric_value),
                COUNT(*)
            FROM observations
            WHERE layer IN ('weather', 'power_grid', 'hydro', 'buoy', 'space_weather', 'air_quality')
              AND timestamp < ?
              AND metric_value IS NOT NULL
              AND metric_name IS NOT NULL
            GROUP BY layer, provider, entity_id, metric_name, (timestamp / 3600000) * 3600000
            "#,
        )
        .bind(metric_14d_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Hourly rollup insertion failed: {}", e)))?
        .rows_affected();
        report.metric_series_rolled_up = rolled_up;

        // Purge raw metric observations older than 14 days (now safely in rollup table)
        let metric_raw_purged = sqlx::query(
            "DELETE FROM observations WHERE layer IN ('weather', 'power_grid', 'hydro', 'buoy', 'space_weather', 'air_quality') AND timestamp < ?",
        )
        .bind(metric_14d_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Raw metric purge failed: {}", e)))?
        .rows_affected();
        report.metric_series_raw_purged = metric_raw_purged;

        // Prune metric rollups older than 365 days
        let rollup_365d_cutoff = now_ms - (365 * 86400 * 1000);
        let _ = sqlx::query("DELETE FROM metric_rollups_hourly WHERE timestamp_hour < ?")
            .bind(rollup_365d_cutoff)
            .execute(&self.pool)
            .await;

        // 4. Discrete Events: Earthquakes, Wildfires (> 180 days)
        let discrete_cutoff = now_ms - (180 * 86400 * 1000);
        let discrete_rows = sqlx::query(
            "DELETE FROM observations WHERE layer IN ('quake', 'wildfire') AND timestamp < ?",
        )
        .bind(discrete_cutoff)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Discrete events sweep failed: {}", e)))?
        .rows_affected();
        report.discrete_events_purged = discrete_rows;

        // 5. 10 GB Disk Cap Trigger (Reclamation at 90% capacity)
        let current_size = self.get_database_size_bytes().await.unwrap_or(0);
        let threshold_90_pct = (self.disk_cap_bytes as f64 * 0.90) as u64;

        if current_size >= threshold_90_pct {
            warn!(
                "Database size ({} bytes) breached 90% of cap ({} bytes). Triggering aggressive FIFO reclamation...",
                current_size, self.disk_cap_bytes
            );
            report.disk_cap_triggered = true;

            // Aggressive step A: Truncate Live Tracking from 48h down to 24h
            let tracking_24h_cutoff = now_ms - (24 * 3600 * 1000);
            let _ = sqlx::query(
                "DELETE FROM observations WHERE layer IN ('aircraft', 'ship', 'satellite') AND timestamp < ?",
            )
            .bind(tracking_24h_cutoff)
            .execute(&self.pool)
            .await;

            // Aggressive step B: Truncate Metric Series rollups from 365d down to 180d
            let rollup_180d_cutoff = now_ms - (180 * 86400 * 1000);
            let _ = sqlx::query("DELETE FROM metric_rollups_hourly WHERE timestamp_hour < ?")
                .bind(rollup_180d_cutoff)
                .execute(&self.pool)
                .await;

            // Aggressive step C: Purge oldest discrete events if still over 90%
            let size_after = self.get_database_size_bytes().await.unwrap_or(0);
            if size_after >= threshold_90_pct {
                let _ = sqlx::query(
                    r#"
                    DELETE FROM observations
                    WHERE id IN (
                        SELECT id FROM observations
                        WHERE layer IN ('quake', 'wildfire')
                        ORDER BY timestamp ASC
                        LIMIT 5000
                    )
                    "#,
                )
                .execute(&self.pool)
                .await;
            }

            // Compact to reclaim OS space
            let _ = self.compact_database().await;
        }

        report.final_size_bytes = self.get_database_size_bytes().await.unwrap_or(0);

        info!(
            "Retention sweep complete: {} ephem, {} downsampled, {} tracking purged, {} rollups created, {} raw metrics purged, cap_triggered: {}",
            report.ephemeral_purged,
            report.live_tracking_downsampled,
            report.live_tracking_purged,
            report.metric_series_rolled_up,
            report.metric_series_raw_purged,
            report.disk_cap_triggered
        );

        Ok(report)
    }

    /// Truncates ephemeral news, alerts, and dead-reckoned tracking points.
    pub async fn clear_caches(&self) -> Result<u64> {
        let rows = sqlx::query(
            "DELETE FROM observations WHERE layer IN ('news', 'alert') OR is_interpolated = 1",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to clear caches: {}", e)))?
        .rows_affected();

        info!("Cleared caches: {} records removed", rows);
        Ok(rows)
    }

    /// Executes SQLite VACUUM to reclaim free database pages.
    pub async fn compact_database(&self) -> Result<()> {
        info!("Executing database VACUUM...");
        sqlx::query("VACUUM;")
            .execute(&self.pool)
            .await
            .map_err(|e| HumboldtError::Storage(format!("VACUUM failed: {}", e)))?;
        info!("Database compaction complete.");
        Ok(())
    }

    /// Calculates current database file size in bytes using SQLite page count PRAGMAs.
    pub async fn get_database_size_bytes(&self) -> Result<u64> {
        let page_count_row = sqlx::query("PRAGMA page_count;")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to read page_count: {}", e)))?;
        let page_count: i64 = page_count_row.get(0);

        let page_size_row = sqlx::query("PRAGMA page_size;")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to read page_size: {}", e)))?;
        let page_size: i64 = page_size_row.get(0);

        Ok((page_count.max(0) as u64) * (page_size.max(0) as u64))
    }
}
