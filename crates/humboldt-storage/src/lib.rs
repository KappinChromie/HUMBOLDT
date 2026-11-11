pub mod migrations;
pub mod observations;
pub mod pool;
pub mod retention;
pub mod snapshots;

pub use migrations::run_migrations;
pub use observations::ObservationStore;
pub use pool::{create_in_memory_pool, create_sqlite_pool};
pub use retention::{RetentionManager, RetentionReport, DEFAULT_DISK_CAP_BYTES};
pub use snapshots::SnapshotStore;

use humboldt_core::Result;
use sqlx::SqlitePool;
use std::path::Path;

/// Unified database handle for the HUMBOLDT storage subsystem.
#[derive(Clone)]
pub struct Storage {
    pub pool: SqlitePool,
    pub observations: ObservationStore,
    pub snapshots: SnapshotStore,
    pub retention: RetentionManager,
}

impl Storage {
    /// Opens or creates the SQLite database at `db_path`, running all migrations.
    pub async fn open(db_path: &Path) -> Result<Self> {
        let pool = create_sqlite_pool(db_path).await?;
        run_migrations(&pool).await?;

        Ok(Self {
            observations: ObservationStore::new(pool.clone()),
            snapshots: SnapshotStore::new(pool.clone()),
            retention: RetentionManager::new(pool.clone()),
            pool,
        })
    }

    /// Creates an in-memory storage instance with migrated schema for testing.
    pub async fn in_memory() -> Result<Self> {
        let pool = create_in_memory_pool().await?;
        run_migrations(&pool).await?;

        Ok(Self {
            observations: ObservationStore::new(pool.clone()),
            snapshots: SnapshotStore::new(pool.clone()),
            retention: RetentionManager::new(pool.clone()),
            pool,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use humboldt_core::{LayerType, Observation, Snapshot};
    use sqlx::Row;

    #[tokio::test]
    async fn test_storage_initialization_and_observations() {
        let storage = Storage::in_memory().await.expect("Failed to init storage");

        let obs = Observation {
            id: None,
            timestamp: 1731200000000,
            layer: LayerType::Quake,
            provider: "usgs_quakes".to_string(),
            entity_id: Some("us7000test".to_string()),
            latitude: Some(-5.312),
            longitude: Some(152.845),
            altitude_m: Some(-52400.0),
            metric_name: Some("magnitude".to_string()),
            metric_value: Some(6.2),
            attributes: Some(serde_json::json!({ "place": "Papua New Guinea" })),
            is_interpolated: false,
        };

        let row_id = storage.observations.insert(&obs).await.expect("Insert failed");
        assert!(row_id > 0);

        let latest = storage
            .observations
            .query_latest(Some("quake"), 10)
            .await
            .expect("Query failed");
        assert_eq!(latest.len(), 1);
        assert_eq!(latest[0].entity_id.as_deref(), Some("us7000test"));
        assert_eq!(latest[0].metric_value, Some(6.2));
    }

    #[tokio::test]
    async fn test_snapshot_lifecycle() {
        let storage = Storage::in_memory().await.expect("Failed to init storage");

        let snap = Snapshot {
            id: "test-snap-uuid-1".to_string(),
            name: "KC Temp Oct 2026".to_string(),
            created_at: 1731200000,
            layer: "weather".to_string(),
            metric_name: "temp_c".to_string(),
            query_entity: Some("Kansas City".to_string()),
            start_time: 1730000000,
            end_time: 1731200000,
            resolution: "1h_rollup".to_string(),
            metadata_json: "{}".to_string(),
            data_points: vec![1, 2, 3, 4, 5],
        };

        storage.snapshots.save(&snap).await.expect("Save failed");

        let loaded = storage
            .snapshots
            .get("test-snap-uuid-1")
            .await
            .expect("Get failed")
            .expect("Snapshot should exist");

        assert_eq!(loaded.name, "KC Temp Oct 2026");
        assert_eq!(loaded.data_points, vec![1, 2, 3, 4, 5]);

        let all = storage.snapshots.list().await.expect("List failed");
        assert_eq!(all.len(), 1);

        let deleted = storage
            .snapshots
            .delete("test-snap-uuid-1")
            .await
            .expect("Delete failed");
        assert!(deleted);

        let reload = storage
            .snapshots
            .get("test-snap-uuid-1")
            .await
            .expect("Get failed");
        assert!(reload.is_none());
    }

    #[tokio::test]
    async fn test_retention_sweep_pruning() {
        let storage = Storage::in_memory().await.expect("Failed to init storage");
        let now_ms = 1731200000000i64;

        // Old ephemeral observation (3 hours ago) -> should be purged
        let old_news = Observation {
            id: None,
            timestamp: now_ms - (3 * 3600 * 1000),
            layer: LayerType::News,
            provider: "gdelt_doc".to_string(),
            entity_id: None,
            latitude: None,
            longitude: None,
            altitude_m: None,
            metric_name: None,
            metric_value: None,
            attributes: None,
            is_interpolated: false,
        };

        // Fresh ephemeral observation (30 minutes ago) -> should be kept
        let fresh_news = Observation {
            id: None,
            timestamp: now_ms - (30 * 60 * 1000),
            layer: LayerType::News,
            provider: "gdelt_doc".to_string(),
            entity_id: None,
            latitude: None,
            longitude: None,
            altitude_m: None,
            metric_name: None,
            metric_value: None,
            attributes: None,
            is_interpolated: false,
        };

        storage.observations.insert(&old_news).await.unwrap();
        storage.observations.insert(&fresh_news).await.unwrap();

        let report = storage.retention.run_sweep(now_ms).await.unwrap();
        assert_eq!(report.ephemeral_purged, 1);

        let remaining = storage
            .observations
            .query_latest(Some("news"), 10)
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].timestamp, fresh_news.timestamp);
    }

    #[tokio::test]
    async fn test_metric_rollups_and_live_downsampling() {
        let storage = Storage::in_memory().await.expect("Failed to init storage");
        let now_ms = 1731200000000i64;

        // 1. Insert multiple live tracking positions in the same minute (3 hours ago)
        let tracking_base_time = now_ms - (3 * 3600 * 1000);
        let sat_pos1 = Observation {
            id: None,
            timestamp: tracking_base_time,
            layer: LayerType::Satellite,
            provider: "celestrak".to_string(),
            entity_id: Some("25544".to_string()),
            latitude: Some(10.0),
            longitude: Some(20.0),
            altitude_m: Some(420000.0),
            metric_name: None,
            metric_value: None,
            attributes: None,
            is_interpolated: false,
        };
        let sat_pos2 = Observation {
            id: None,
            timestamp: tracking_base_time + 15000, // 15 seconds later (same minute)
            layer: LayerType::Satellite,
            provider: "celestrak".to_string(),
            entity_id: Some("25544".to_string()),
            latitude: Some(10.1),
            longitude: Some(20.1),
            altitude_m: Some(420000.0),
            metric_name: None,
            metric_value: None,
            attributes: None,
            is_interpolated: false,
        };
        storage.observations.insert(&sat_pos1).await.unwrap();
        storage.observations.insert(&sat_pos2).await.unwrap();

        // 2. Insert metric series observations older than 14 days (floored to hour)
        let old_metric_time = ((now_ms - (15 * 86400 * 1000)) / 3600000) * 3600000;
        let weather1 = Observation {
            id: None,
            timestamp: old_metric_time,
            layer: LayerType::Weather,
            provider: "open_meteo".to_string(),
            entity_id: Some("station_kc".to_string()),
            latitude: Some(39.09),
            longitude: Some(-94.57),
            altitude_m: None,
            metric_name: Some("temp_c".to_string()),
            metric_value: Some(20.0),
            attributes: None,
            is_interpolated: false,
        };
        let weather2 = Observation {
            id: None,
            timestamp: old_metric_time + 600000, // 10 minutes later (same hour)
            layer: LayerType::Weather,
            provider: "open_meteo".to_string(),
            entity_id: Some("station_kc".to_string()),
            latitude: Some(39.09),
            longitude: Some(-94.57),
            altitude_m: None,
            metric_name: Some("temp_c".to_string()),
            metric_value: Some(24.0),
            attributes: None,
            is_interpolated: false,
        };
        storage.observations.insert(&weather1).await.unwrap();
        storage.observations.insert(&weather2).await.unwrap();

        // Run retention sweep
        let report = storage.retention.run_sweep(now_ms).await.unwrap();
        assert_eq!(report.live_tracking_downsampled, 1, "Duplicate in same minute should be downsampled");
        assert_eq!(report.metric_series_rolled_up, 1, "Two weather readings in same hour should rollup to 1 hourly row");
        assert_eq!(report.metric_series_raw_purged, 2, "Raw weather readings should be purged");

        // Verify rollup values
        let rollup_row = sqlx::query("SELECT avg_value, min_value, max_value, sample_count FROM metric_rollups_hourly WHERE metric_name = 'temp_c'")
            .fetch_one(&storage.pool)
            .await
            .unwrap();
        let avg: f64 = rollup_row.get("avg_value");
        let min: f64 = rollup_row.get("min_value");
        let max: f64 = rollup_row.get("max_value");
        let count: i32 = rollup_row.get("sample_count");

        assert_eq!(avg, 22.0);
        assert_eq!(min, 20.0);
        assert_eq!(max, 24.0);
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_disk_cap_90_percent_trigger() {
        let storage = Storage::in_memory().await.expect("Failed to init storage");
        let now_ms = 1731200000000i64;

        // Configure a small retention manager with a 1 KB disk cap to trigger 90% reclamation
        let small_retention = RetentionManager::with_disk_cap(storage.pool.clone(), 1024);
        let report = small_retention.run_sweep(now_ms).await.unwrap();
        assert!(report.disk_cap_triggered, "Should trigger disk cap reclamation when size exceeds 90% of cap");
    }
}
