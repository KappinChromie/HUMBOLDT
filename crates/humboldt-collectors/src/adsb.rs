use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_ADSB_LOL_URL: &str = "https://api.adsb.lol/v2/lat/39.0997/lon/-94.5786/dist/250";
pub const DEFAULT_AIRPLANES_LIVE_URL: &str = "https://api.airplanes.live/v2/point/39.0997/-94.5786/250";

#[derive(Debug, Deserialize)]
struct AdsbResponse {
    pub ac: Option<Vec<AdsbAircraft>>,
    pub now: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct AdsbAircraft {
    pub hex: String,
    pub flight: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub alt_baro: Option<serde_json::Value>,
    pub track: Option<f64>,
    pub gs: Option<f64>,
    pub squawk: Option<String>,
    pub category: Option<String>,
}

/// Parses ADS-B JSON response into normalized Observation records.
pub fn parse_adsb_json(json_str: &str, provider_slug: &str) -> Result<Vec<Observation>> {
    let parsed: AdsbResponse = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let aircraft_list = match parsed.ac {
        Some(list) => list,
        None => return Ok(Vec::new()),
    };

    let now_ms = parsed
        .now
        .map(|sec| (sec * 1000.0) as i64)
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    let mut observations = Vec::with_capacity(aircraft_list.len());

    for ac in aircraft_list {
        if let (Some(lat), Some(lon)) = (ac.lat, ac.lon) {
            // Altitude parsing (alt_baro can be a number or "ground")
            let alt_m = match ac.alt_baro {
                Some(serde_json::Value::Number(n)) => n.as_f64().map(|ft| ft * 0.3048),
                _ => Some(0.0),
            };

            let callsign = ac.flight.map(|s| s.trim().to_string());
            let attributes = serde_json::json!({
                "callsign": callsign,
                "track_deg": ac.track,
                "ground_speed_knots": ac.gs,
                "squawk": ac.squawk,
                "category": ac.category,
            });

            let obs = Observation {
                id: None,
                timestamp: now_ms,
                layer: LayerType::Aircraft,
                provider: provider_slug.to_string(),
                entity_id: Some(ac.hex.to_lowercase()),
                latitude: Some(lat),
                longitude: Some(lon),
                altitude_m: alt_m,
                metric_name: Some("altitude_m".to_string()),
                metric_value: alt_m,
                attributes: Some(attributes),
                is_interpolated: false,
            };

            if obs.is_valid_geo() {
                observations.push(obs);
            }
        }
    }

    Ok(observations)
}

/// Fetches ADS-B data from an HTTP provider endpoint.
pub async fn fetch_adsb(
    client: &reqwest::Client,
    url: &str,
    provider_slug: &str,
) -> Result<Vec<Observation>> {
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("ADS-B request to {} failed: {}", url, e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "ADS-B HTTP error from {}: status {}",
            provider_slug,
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read ADS-B body: {}", e)))?;

    let aircraft = parse_adsb_json(&body, provider_slug)?;
    info!(
        "Ingested {} aircraft from ADS-B provider '{}'.",
        aircraft.len(),
        provider_slug
    );
    Ok(aircraft)
}
