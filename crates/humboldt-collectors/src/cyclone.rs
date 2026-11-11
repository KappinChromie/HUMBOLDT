use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_NOAA_NHC_STORMS_URL: &str = "https://www.nhc.noaa.gov/CurrentStorms.json";

/// Parses NOAA NHC tropical cyclone feeds (activeStorms format or GeoJSON FeatureCollection)
/// into normalized HUMBOLDT Observation records.
pub fn parse_nhc_cyclones_json(json_str: &str) -> Result<Vec<Observation>> {
    let root: Value = serde_json::from_str(json_str).map_err(HumboldtError::Serialization)?;
    let mut observations = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // 1. Check for standard activeStorms array
    if let Some(storms) = root.get("activeStorms").and_then(|v| v.as_array()) {
        for storm in storms {
            let id = storm.get("id").and_then(|v| v.as_str()).unwrap_or("storm");
            let name = storm.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed Cyclone");
            let classification = storm.get("classification").and_then(|v| v.as_str()).unwrap_or("TS");
            let lat = storm.get("latitude").and_then(|v| v.as_f64());
            let lon = storm.get("longitude").and_then(|v| v.as_f64());
            let intensity_kts = storm.get("intensity").and_then(|v| v.as_f64());
            let pressure_mb = storm.get("pressure").and_then(|v| v.as_f64());
            let movement_dir = storm.get("movementDir").and_then(|v| v.as_f64());
            let movement_speed = storm.get("movementSpeed").and_then(|v| v.as_f64());

            let mut attrs = serde_json::Map::new();
            attrs.insert("name".to_string(), Value::String(name.to_string()));
            attrs.insert("classification".to_string(), Value::String(classification.to_string()));
            if let Some(p) = pressure_mb {
                attrs.insert("pressure_mb".to_string(), Value::from(p));
            }
            if let Some(dir) = movement_dir {
                attrs.insert("movement_heading_deg".to_string(), Value::from(dir));
            }
            if let Some(spd) = movement_speed {
                attrs.insert("movement_speed_kts".to_string(), Value::from(spd));
            }

            observations.push(Observation {
                id: None,
                timestamp: now,
                layer: LayerType::Cyclone,
                provider: "noaa_nhc".to_string(),
                entity_id: Some(id.to_string()),
                latitude: lat,
                longitude: lon,
                altitude_m: Some(0.0),
                metric_name: Some("wind_speed_knots".to_string()),
                metric_value: intensity_kts,
                attributes: Some(Value::Object(attrs)),
                is_interpolated: false,
            });
        }
    } else if let Some(features) = root.get("features").and_then(|v| v.as_array()) {
        // 2. Fallback to GeoJSON FeatureCollection
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

            let storm_id = props.get("STORMID")
                .or_else(|| props.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("cyclone");

            let storm_name = props.get("STORMNAM")
                .or_else(|| props.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Tropical Cyclone");

            let intensity = props.get("INTENSITY")
                .or_else(|| props.get("max_wind_kts"))
                .and_then(|v| v.as_f64());

            let pressure = props.get("MSLP")
                .or_else(|| props.get("pressure_mb"))
                .and_then(|v| v.as_f64());

            observations.push(Observation {
                id: None,
                timestamp: now,
                layer: LayerType::Cyclone,
                provider: "noaa_nhc".to_string(),
                entity_id: Some(storm_id.to_string()),
                latitude: lat,
                longitude: lon,
                altitude_m: Some(0.0),
                metric_name: Some("wind_speed_knots".to_string()),
                metric_value: intensity,
                attributes: Some(serde_json::json!({
                    "name": storm_name,
                    "pressure_mb": pressure,
                    "classification": props.get("STORMTYPE").and_then(|v| v.as_str()).unwrap_or("TC"),
                })),
                is_interpolated: false,
            });
        }
    }

    Ok(observations)
}

/// Fetches active cyclones from NOAA NHC.
pub async fn fetch_noaa_nhc_cyclones(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_NOAA_NHC_STORMS_URL);
    info!("Fetching active tropical cyclones from NOAA NHC: {}", url);

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("NOAA NHC HTTP request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "NOAA NHC responded with HTTP {}",
            resp.status()
        )));
    }

    let body = resp.text().await.map_err(|e| HumboldtError::Storage(format!("Failed to read NOAA NHC body: {}", e)))?;
    parse_nhc_cyclones_json(&body)
}
