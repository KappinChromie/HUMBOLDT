use chrono::NaiveDateTime;
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_NOAA_SWPC_KP_URL: &str =
    "https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json";

/// Parses NOAA SWPC Planetary K-Index array into normalized Observation records.
pub fn parse_noaa_kp_json(json_str: &str) -> Result<Vec<Observation>> {
    let rows: Vec<Vec<String>> = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let mut observations = Vec::new();

    // Skip header row: ["time_tag", "kp", "a_running", "station_count"]
    for row in rows.into_iter().skip(1) {
        if row.len() < 2 {
            continue;
        }

        let time_tag = &row[0];
        let kp_str = &row[1];

        if let Ok(kp_val) = kp_str.parse::<f64>() {
            let timestamp = NaiveDateTime::parse_from_str(time_tag, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc().timestamp_millis())
                .unwrap_or_else(|_| {
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as i64
                });

            let a_running = row.get(2).and_then(|s| s.parse::<f64>().ok());
            let station_count = row.get(3).and_then(|s| s.parse::<i32>().ok());

            let attributes = serde_json::json!({
                "time_tag": time_tag,
                "a_running": a_running,
                "station_count": station_count,
            });

            observations.push(Observation {
                id: None,
                timestamp,
                layer: LayerType::SpaceWeather,
                provider: "noaa_swpc".to_string(),
                entity_id: Some("planetary_kp".to_string()),
                latitude: None,
                longitude: None,
                altitude_m: None,
                metric_name: Some("kp_index".to_string()),
                metric_value: Some(kp_val),
                attributes: Some(attributes),
                is_interpolated: false,
            });
        }
    }

    Ok(observations)
}

/// Fetches Planetary K-Index observations from NOAA SWPC.
pub async fn fetch_noaa_kp(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_NOAA_SWPC_KP_URL);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("NOAA SWPC HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "NOAA SWPC HTTP error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read NOAA body: {}", e)))?;

    let obs = parse_noaa_kp_json(&body)?;
    info!("Ingested {} Space Weather Kp readings from NOAA SWPC.", obs.len());
    Ok(obs)
}
