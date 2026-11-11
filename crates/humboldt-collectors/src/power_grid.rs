use chrono::DateTime;
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_UK_CARBON_INTENSITY_URL: &str = "https://api.carbonintensity.org.uk/generation";

#[derive(Debug, Deserialize)]
struct UkGenerationResponse {
    pub data: Option<UkGenerationData>,
}

#[derive(Debug, Deserialize)]
struct UkGenerationData {
    pub from: Option<String>,
    pub to: Option<String>,
    pub generationmix: Option<Vec<UkFuelMix>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct UkFuelMix {
    pub fuel: String,
    pub perc: f64,
}

/// Parses UK National Grid generation mix into normalized Observation records.
pub fn parse_uk_power_grid_json(json_str: &str) -> Result<Vec<Observation>> {
    let resp: UkGenerationResponse = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let data = match resp.data {
        Some(d) => d,
        None => return Ok(Vec::new()),
    };

    let timestamp = data
        .to
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64
        });

    let fuel_list = data.generationmix.unwrap_or_default();
    let mut observations = Vec::with_capacity(fuel_list.len());

    // Compute renewable vs fossil percentage
    let mut renewable_perc = 0.0;
    let mut fossil_perc = 0.0;

    for mix in &fuel_list {
        match mix.fuel.as_str() {
            "wind" | "solar" | "hydro" | "biomass" => renewable_perc += mix.perc,
            "gas" | "coal" => fossil_perc += mix.perc,
            _ => {}
        }
    }

    let attributes = serde_json::json!({
        "from": data.from,
        "to": data.to,
        "renewable_total_perc": renewable_perc,
        "fossil_total_perc": fossil_perc,
        "mix": fuel_list,
    });

    let obs = Observation {
        id: None,
        timestamp,
        layer: LayerType::PowerGrid,
        provider: "uk_carbon_intensity".to_string(),
        entity_id: Some("UK_NATIONAL_GRID".to_string()),
        latitude: Some(54.5), // Center of UK
        longitude: Some(-2.5),
        altitude_m: Some(0.0),
        metric_name: Some("renewable_gen_pct".to_string()),
        metric_value: Some(renewable_perc),
        attributes: Some(attributes),
        is_interpolated: false,
    };

    observations.push(obs);
    Ok(observations)
}

/// Fetches generation mix from UK National Grid API.
pub async fn fetch_uk_power_grid(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_UK_CARBON_INTENSITY_URL);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("UK Grid HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "UK Grid HTTP error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read UK Grid body: {}", e)))?;

    let obs = parse_uk_power_grid_json(&body)?;
    info!("Ingested UK generation mix (renewable: {}%).", obs.first().and_then(|o| o.metric_value).unwrap_or(0.0));
    Ok(obs)
}
