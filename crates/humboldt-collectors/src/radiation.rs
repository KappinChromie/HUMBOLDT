use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_SAFECAST_API_URL: &str = "https://api.safecast.org/measurements.json?limit=100";

/// Parses Safecast radiation measurement records into normalized HUMBOLDT Observation records.
/// Normalizes readings to microSieverts per hour (uSv/h).
pub fn parse_safecast_radiation_json(json_str: &str) -> Result<Vec<Observation>> {
    let root: Value = serde_json::from_str(json_str).map_err(HumboldtError::Serialization)?;
    let mut observations = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let items = if let Some(arr) = root.as_array() {
        arr
    } else if let Some(arr) = root.get("measurements").and_then(|v| v.as_array()) {
        arr
    } else {
        return Ok(observations);
    };

    for item in items {
        let lat = item.get("latitude").and_then(|v| v.as_f64());
        let lon = item.get("longitude").and_then(|v| v.as_f64());

        if lat.is_none() || lon.is_none() {
            continue;
        }

        let raw_val = item.get("value").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let unit = item.get("unit").and_then(|v| v.as_str()).unwrap_or("cpm").to_lowercase();

        // Convert CPM to uSv/h using standard bGeigie Cs-137 calibration factor (334 CPM ~ 1.0 uSv/h)
        let usv_h = if unit.contains("cpm") {
            raw_val / 334.0
        } else if unit.contains("usv") {
            raw_val
        } else {
            raw_val / 334.0
        };

        let sensor_id = item.get("id")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.as_i64().map(|n| n.to_string())
                }
            })
            .unwrap_or_else(|| "sensor".to_string());

        let device_id = item.get("device_id")
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "bGeigie".to_string());

        let captured_at = item.get("captured_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let attrs = serde_json::json!({
            "sensor_id": sensor_id,
            "device_id": device_id,
            "raw_value": raw_val,
            "unit": unit,
            "usv_per_hour": (usv_h * 1000.0).round() / 1000.0,
            "captured_at": captured_at,
        });

        observations.push(Observation {
            id: None,
            timestamp: now,
            layer: LayerType::Radiation,
            provider: "safecast".to_string(),
            entity_id: Some(sensor_id),
            latitude: lat,
            longitude: lon,
            altitude_m: Some(1.0),
            metric_name: Some("radiation_usv_h".to_string()),
            metric_value: Some((usv_h * 1000.0).round() / 1000.0),
            attributes: Some(attrs),
            is_interpolated: false,
        });
    }

    Ok(observations)
}

/// Fetches recent radiation observations from Safecast API.
pub async fn fetch_safecast_radiation(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_SAFECAST_API_URL);
    info!("Fetching ambient radiation measurements from Safecast: {}", url);

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Safecast API HTTP request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "Safecast API responded with HTTP {}",
            resp.status()
        )));
    }

    let body = resp.text().await.map_err(|e| HumboldtError::Storage(format!("Failed to read Safecast body: {}", e)))?;
    parse_safecast_radiation_json(&body)
}
