use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_GDACS_DISASTERS_URL: &str =
    "https://www.gdacs.org/gdacsapi/api/events/geteventlist/SEARCH?eventtypes=EQ,TC,FL,VO,DR&alertlevel=green;orange;red";

/// Parses GDACS GeoJSON disaster events into normalized HUMBOLDT Observation records.
pub fn parse_gdacs_disasters_geojson(json_str: &str) -> Result<Vec<Observation>> {
    let root: Value = serde_json::from_str(json_str).map_err(HumboldtError::Serialization)?;
    let mut observations = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    let features = root.get("features")
        .and_then(|v| v.as_array())
        .ok_or_else(|| HumboldtError::Serialization(serde::de::Error::custom("Missing 'features' array")))?;

    for feature in features {
        let props = feature.get("properties").unwrap_or(&Value::Null);
        let geom = feature.get("geometry").unwrap_or(&Value::Null);

        let (lon, lat) = match geom.get("coordinates").and_then(|c| c.as_array()) {
            Some(coords) if coords.len() >= 2 => (
                coords[0].as_f64(),
                coords[1].as_f64(),
            ),
            _ => (None, None),
        };

        if lat.is_none() || lon.is_none() {
            continue;
        }

        let event_id = props.get("eventid")
            .or_else(|| props.get("id"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.as_i64().map(|n| n.to_string())
                }
            })
            .unwrap_or_else(|| "disaster".to_string());

        let event_type = props.get("eventtype")
            .or_else(|| props.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("DISASTER");

        let event_name = props.get("name")
            .or_else(|| props.get("eventname"))
            .or_else(|| props.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("Humanitarian / Disaster Event");

        let alert_level = props.get("alertlevel")
            .or_else(|| props.get("severity"))
            .and_then(|v| v.as_str())
            .unwrap_or("Orange");

        let alert_score = props.get("alertscore")
            .and_then(|v| v.as_f64())
            .unwrap_or(match alert_level.to_lowercase().as_str() {
                "red" => 3.0,
                "orange" => 2.0,
                _ => 1.0,
            });

        let country = props.get("country")
            .and_then(|v| v.as_str())
            .unwrap_or("Global");

        let description = props.get("description")
            .or_else(|| props.get("htmldescription"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let attrs = serde_json::json!({
            "event_name": event_name,
            "event_type": event_type,
            "alert_level": alert_level,
            "country": country,
            "description": description,
        });

        observations.push(Observation {
            id: None,
            timestamp: now,
            layer: LayerType::Disaster,
            provider: "gdacs".to_string(),
            entity_id: Some(event_id),
            latitude: lat,
            longitude: lon,
            altitude_m: Some(0.0),
            metric_name: Some("alert_score".to_string()),
            metric_value: Some(alert_score),
            attributes: Some(attrs),
            is_interpolated: false,
        });
    }

    Ok(observations)
}

/// Fetches active disaster alerts from GDACS.
pub async fn fetch_gdacs_disasters(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_GDACS_DISASTERS_URL);
    info!("Fetching humanitarian disaster events from GDACS: {}", url);

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("GDACS API HTTP request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "GDACS API responded with HTTP {}",
            resp.status()
        )));
    }

    let body = resp.text().await.map_err(|e| HumboldtError::Storage(format!("Failed to read GDACS body: {}", e)))?;
    parse_gdacs_disasters_geojson(&body)
}
