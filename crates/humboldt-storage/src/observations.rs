use humboldt_core::{HumboldtError, LayerType, Observation, Result};
use sqlx::{Row, SqlitePool};

/// Ingestion and retrieval queries for the `observations` table.
#[derive(Clone)]
pub struct ObservationStore {
    pool: SqlitePool,
}

impl ObservationStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Inserts a single observation record and returns its generated row id.
    pub async fn insert(&self, obs: &Observation) -> Result<i64> {
        let layer_str = obs.layer.as_str();
        let attrs_str = obs
            .attributes
            .as_ref()
            .map(|a| serde_json::to_string(a).unwrap_or_default());

        let id = sqlx::query(
            r#"
            INSERT INTO observations (
                timestamp, layer, provider, entity_id, latitude, longitude,
                altitude_m, metric_name, metric_value, attributes, is_interpolated
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(obs.timestamp)
        .bind(layer_str)
        .bind(&obs.provider)
        .bind(&obs.entity_id)
        .bind(obs.latitude)
        .bind(obs.longitude)
        .bind(obs.altitude_m)
        .bind(&obs.metric_name)
        .bind(obs.metric_value)
        .bind(attrs_str)
        .bind(if obs.is_interpolated { 1 } else { 0 })
        .execute(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to insert observation: {}", e)))?
        .last_insert_rowid();

        Ok(id)
    }

    /// Inserts a batch of observations inside an atomic transaction.
    pub async fn insert_batch(&self, batch: &[Observation]) -> Result<usize> {
        if batch.is_empty() {
            return Ok(0);
        }

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to start transaction: {}", e)))?;

        for obs in batch {
            let layer_str = obs.layer.as_str();
            let attrs_str = obs
                .attributes
                .as_ref()
                .map(|a| serde_json::to_string(a).unwrap_or_default());

            sqlx::query(
                r#"
                INSERT INTO observations (
                    timestamp, layer, provider, entity_id, latitude, longitude,
                    altitude_m, metric_name, metric_value, attributes, is_interpolated
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(obs.timestamp)
            .bind(layer_str)
            .bind(&obs.provider)
            .bind(&obs.entity_id)
            .bind(obs.latitude)
            .bind(obs.longitude)
            .bind(obs.altitude_m)
            .bind(&obs.metric_name)
            .bind(obs.metric_value)
            .bind(attrs_str)
            .bind(if obs.is_interpolated { 1 } else { 0 })
            .execute(&mut *tx)
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to insert batch row: {}", e)))?;
        }

        tx.commit()
            .await
            .map_err(|e| HumboldtError::Storage(format!("Failed to commit batch: {}", e)))?;

        Ok(batch.len())
    }

    /// Queries latest observations, optionally filtered by layer slug.
    pub async fn query_latest(&self, layer: Option<&str>, limit: u32) -> Result<Vec<Observation>> {
        let rows = if let Some(l) = layer {
            sqlx::query(
                r#"
                SELECT id, timestamp, layer, provider, entity_id, latitude, longitude,
                       altitude_m, metric_name, metric_value, attributes, is_interpolated
                FROM observations
                WHERE layer = ?
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
            )
            .bind(l)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT id, timestamp, layer, provider, entity_id, latitude, longitude,
                       altitude_m, metric_name, metric_value, attributes, is_interpolated
                FROM observations
                ORDER BY timestamp DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| HumboldtError::Storage(format!("Failed to query observations: {}", e)))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let layer_raw: String = row.get("layer");
            let layer: LayerType = serde_json::from_value(serde_json::Value::String(layer_raw.clone()))
                .unwrap_or(LayerType::Weather);

            let attrs_raw: Option<String> = row.get("attributes");
            let attributes: Option<serde_json::Value> = attrs_raw.and_then(|s| serde_json::from_str(&s).ok());

            let is_interpolated_int: i32 = row.get("is_interpolated");

            results.push(Observation {
                id: Some(row.get("id")),
                timestamp: row.get("timestamp"),
                layer,
                provider: row.get("provider"),
                entity_id: row.get("entity_id"),
                latitude: row.get("latitude"),
                longitude: row.get("longitude"),
                altitude_m: row.get("altitude_m"),
                metric_name: row.get("metric_name"),
                metric_value: row.get("metric_value"),
                attributes,
                is_interpolated: is_interpolated_int != 0,
            });
        }

        Ok(results)
    }

    /// Queries observations for a specific entity ID within a time window.
    pub async fn query_by_entity(
        &self,
        entity_id: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Observation>> {
        let rows = sqlx::query(
            r#"
            SELECT id, timestamp, layer, provider, entity_id, latitude, longitude,
                   altitude_m, metric_name, metric_value, attributes, is_interpolated
            FROM observations
            WHERE entity_id = ? AND timestamp >= ? AND timestamp <= ?
            ORDER BY timestamp ASC
            "#,
        )
        .bind(entity_id)
        .bind(start_time)
        .bind(end_time)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to query entity history: {}", e)))?;

        let mut results = Vec::with_capacity(rows.len());
        for row in rows {
            let layer_raw: String = row.get("layer");
            let layer: LayerType = serde_json::from_value(serde_json::Value::String(layer_raw.clone()))
                .unwrap_or(LayerType::Weather);

            let attrs_raw: Option<String> = row.get("attributes");
            let attributes: Option<serde_json::Value> = attrs_raw.and_then(|s| serde_json::from_str(&s).ok());
            let is_interpolated_int: i32 = row.get("is_interpolated");

            results.push(Observation {
                id: Some(row.get("id")),
                timestamp: row.get("timestamp"),
                layer,
                provider: row.get("provider"),
                entity_id: row.get("entity_id"),
                latitude: row.get("latitude"),
                longitude: row.get("longitude"),
                altitude_m: row.get("altitude_m"),
                metric_name: row.get("metric_name"),
                metric_value: row.get("metric_value"),
                attributes,
                is_interpolated: is_interpolated_int != 0,
            });
        }

        Ok(results)
    }

    /// Queries historical scalar time series from observations and hourly rollups.
    pub async fn query_metric_series(
        &self,
        layer: &str,
        metric_name: Option<&str>,
        entity_id: Option<&str>,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<humboldt_core::DataPoint>> {
        let rows = if let (Some(m), Some(e)) = (metric_name, entity_id) {
            sqlx::query(
                r#"
                SELECT timestamp, metric_value
                FROM observations
                WHERE layer = ? AND metric_name = ? AND entity_id = ? AND timestamp >= ? AND timestamp <= ? AND metric_value IS NOT NULL
                ORDER BY timestamp ASC
                "#,
            )
            .bind(layer)
            .bind(m)
            .bind(e)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        } else if let Some(m) = metric_name {
            sqlx::query(
                r#"
                SELECT timestamp, metric_value
                FROM observations
                WHERE layer = ? AND metric_name = ? AND timestamp >= ? AND timestamp <= ? AND metric_value IS NOT NULL
                ORDER BY timestamp ASC
                "#,
            )
            .bind(layer)
            .bind(m)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT timestamp, metric_value
                FROM observations
                WHERE layer = ? AND timestamp >= ? AND timestamp <= ? AND metric_value IS NOT NULL
                ORDER BY timestamp ASC
                "#,
            )
            .bind(layer)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| HumboldtError::Storage(format!("Failed to query metric series: {}", e)))?;

        let mut points: Vec<humboldt_core::DataPoint> = rows
            .into_iter()
            .map(|r| humboldt_core::DataPoint {
                timestamp: r.get("timestamp"),
                value: r.get("metric_value"),
            })
            .collect();

        // Also query hourly rollups for older data if available
        let rollup_rows = if let (Some(m), Some(e)) = (metric_name, entity_id) {
            sqlx::query(
                r#"
                SELECT timestamp_hour, avg_value
                FROM metric_rollups_hourly
                WHERE layer = ? AND metric_name = ? AND entity_id = ? AND timestamp_hour >= ? AND timestamp_hour <= ?
                ORDER BY timestamp_hour ASC
                "#,
            )
            .bind(layer)
            .bind(m)
            .bind(e)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        } else if let Some(m) = metric_name {
            sqlx::query(
                r#"
                SELECT timestamp_hour, avg_value
                FROM metric_rollups_hourly
                WHERE layer = ? AND metric_name = ? AND timestamp_hour >= ? AND timestamp_hour <= ?
                ORDER BY timestamp_hour ASC
                "#,
            )
            .bind(layer)
            .bind(m)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"
                SELECT timestamp_hour, avg_value
                FROM metric_rollups_hourly
                WHERE layer = ? AND timestamp_hour >= ? AND timestamp_hour <= ?
                ORDER BY timestamp_hour ASC
                "#,
            )
            .bind(layer)
            .bind(start_time)
            .bind(end_time)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| HumboldtError::Storage(format!("Failed to query metric rollups: {}", e)))?;

        for r in rollup_rows {
            let ts: i64 = r.get("timestamp_hour");
            let val: f64 = r.get("avg_value");
            // Add if not already present from raw observations
            if !points.iter().any(|p| (p.timestamp - ts).abs() < 1800000) {
                points.push(humboldt_core::DataPoint {
                    timestamp: ts,
                    value: val,
                });
            }
        }

        points.sort_by_key(|p| p.timestamp);
        Ok(points)
    }
}

