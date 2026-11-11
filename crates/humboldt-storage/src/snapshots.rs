use humboldt_core::{HumboldtError, Result, Snapshot};
use sqlx::{Row, SqlitePool};

/// Persistence operations for user-frozen analytical snapshots.
#[derive(Clone)]
pub struct SnapshotStore {
    pool: SqlitePool,
}

impl SnapshotStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Saves or updates a snapshot in the database.
    pub async fn save(&self, snap: &Snapshot) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO snapshots (
                id, name, created_at, layer, metric_name, query_entity,
                start_time, end_time, resolution, metadata_json, data_points
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snap.id)
        .bind(&snap.name)
        .bind(snap.created_at)
        .bind(&snap.layer)
        .bind(&snap.metric_name)
        .bind(&snap.query_entity)
        .bind(snap.start_time)
        .bind(snap.end_time)
        .bind(&snap.resolution)
        .bind(&snap.metadata_json)
        .bind(&snap.data_points)
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to save snapshot: {}", e)))?;

        Ok(())
    }

    /// Retrieves a single snapshot by UUID.
    pub async fn get(&self, id: &str) -> Result<Option<Snapshot>> {
        let row_opt = sqlx::query(
            r#"
            SELECT id, name, created_at, layer, metric_name, query_entity,
                   start_time, end_time, resolution, metadata_json, data_points
            FROM snapshots
            WHERE id = ?
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to fetch snapshot: {}", e)))?;

        Ok(row_opt.map(|row| Snapshot {
            id: row.get("id"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            layer: row.get("layer"),
            metric_name: row.get("metric_name"),
            query_entity: row.get("query_entity"),
            start_time: row.get("start_time"),
            end_time: row.get("end_time"),
            resolution: row.get("resolution"),
            metadata_json: row.get("metadata_json"),
            data_points: row.get("data_points"),
        }))
    }

    /// Lists all snapshots ordered by creation date descending.
    pub async fn list(&self) -> Result<Vec<Snapshot>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, created_at, layer, metric_name, query_entity,
                   start_time, end_time, resolution, metadata_json, data_points
            FROM snapshots
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to list snapshots: {}", e)))?;

        let mut list = Vec::with_capacity(rows.len());
        for row in rows {
            list.push(Snapshot {
                id: row.get("id"),
                name: row.get("name"),
                created_at: row.get("created_at"),
                layer: row.get("layer"),
                metric_name: row.get("metric_name"),
                query_entity: row.get("query_entity"),
                start_time: row.get("start_time"),
                end_time: row.get("end_time"),
                resolution: row.get("resolution"),
                metadata_json: row.get("metadata_json"),
                data_points: row.get("data_points"),
            });
        }

        Ok(list)
    }

    /// Deletes a specific snapshot by UUID.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let rows_affected = sqlx::query("DELETE FROM snapshots WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to delete snapshot: {}", e)))?
            .rows_affected();

        Ok(rows_affected > 0)
    }

    /// Deletes all user snapshots.
    pub async fn delete_all(&self) -> Result<u64> {
        let rows_affected = sqlx::query("DELETE FROM snapshots")
            .execute(&self.pool)
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to purge snapshots: {}", e)))?
            .rows_affected();

        Ok(rows_affected)
    }
}
