use chrono::NaiveDateTime;
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug, Deserialize)]
struct OpenMeteoResponse {
    pub latitude: f64,
    pub longitude: f64,
    pub current: Option<OpenMeteoCurrent>,
}

#[derive(Debug, Deserialize)]
struct OpenMeteoCurrent {
    pub time: String,
    pub temperature_2m: Option<f64>,
    pub relative_humidity_2m: Option<f64>,
    pub apparent_temperature: Option<f64>,
    pub precipitation: Option<f64>,
    pub wind_speed_10m: Option<f64>,
}

/// Parses Open-Meteo current weather payload into normalized Observation records.
pub fn parse_open_meteo_json(json_str: &str, entity_label: Option<&str>) -> Result<Vec<Observation>> {
    let resp: OpenMeteoResponse = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let current = match resp.current {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };

    let timestamp = NaiveDateTime::parse_from_str(&current.time, "%Y-%m-%dT%H:%M")
        .map(|ndt| ndt.and_utc().timestamp_millis())
        .unwrap_or_else(|_| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    let entity_id = entity_label
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{:.2}_{:.2}", resp.latitude, resp.longitude));

    let attributes = serde_json::json!({
        "temperature_2m": current.temperature_2m,
        "relative_humidity_2m": current.relative_humidity_2m,
        "apparent_temperature": current.apparent_temperature,
        "precipitation_mm": current.precipitation,
        "wind_speed_10m_kmh": current.wind_speed_10m,
    });

    let obs = Observation {
        id: None,
        timestamp,
        layer: LayerType::Weather,
        provider: "open_meteo".to_string(),
        entity_id: Some(entity_id),
        latitude: Some(resp.latitude),
        longitude: Some(resp.longitude),
        altitude_m: Some(0.0),
        metric_name: Some("temp_c".to_string()),
        metric_value: current.temperature_2m,
        attributes: Some(attributes),
        is_interpolated: false,
    };

    Ok(vec![obs])
}

/// Fetches current weather from Open-Meteo for coordinates.
pub async fn fetch_open_meteo(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
    label: Option<&str>,
) -> Result<Vec<Observation>> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,precipitation,wind_speed_10m",
        lat, lon
    );

    let response = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Open-Meteo HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "Open-Meteo error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read Open-Meteo body: {}", e)))?;

    let obs = parse_open_meteo_json(&body, label)?;
    info!("Ingested weather for location ({}, {}).", lat, lon);
    Ok(obs)
}
