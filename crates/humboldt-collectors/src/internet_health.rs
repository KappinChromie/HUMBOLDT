use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_CAIDA_IODA_URL: &str = "https://api.ioda.inetintel.cc.gatech.edu/v2/outages/macro";

#[derive(Debug, Deserialize)]
struct IodaResponse {
    pub data: Option<Vec<IodaOutage>>,
}

#[derive(Debug, Deserialize)]
struct IodaOutage {
    pub id: Option<String>,
    pub entity_type: Option<String>,
    pub entity_code: Option<String>,
    pub entity_name: Option<String>,
    pub start_time: Option<i64>,
    pub score: Option<f64>,
    pub affected_asns: Option<i32>,
    pub description: Option<String>,
}

/// Parses CAIDA IODA macro outages into normalized Observation records.
pub fn parse_caida_ioda_json(json_str: &str) -> Result<Vec<Observation>> {
    let resp: IodaResponse = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let list = match resp.data {
        Some(l) => l,
        None => return Ok(Vec::new()),
    };

    let mut observations = Vec::with_capacity(list.len());

    for outage in list {
        let timestamp = outage
            .start_time
            .map(|s| s * 1000)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64
            });

        let entity_id = outage
            .entity_code
            .clone()
            .or_else(|| outage.id.clone())
            .unwrap_or_else(|| "ioda_global".to_string());

        let score = outage.score.unwrap_or(0.0);

        let attributes = serde_json::json!({
            "outage_id": outage.id,
            "entity_type": outage.entity_type,
            "entity_name": outage.entity_name,
            "affected_asns": outage.affected_asns,
            "description": outage.description,
        });

        let obs = Observation {
            id: None,
            timestamp,
            layer: LayerType::InternetHealth,
            provider: "caida_ioda".to_string(),
            entity_id: Some(entity_id),
            latitude: None,
            longitude: None,
            altitude_m: None,
            metric_name: Some("bgp_disruption_score".to_string()),
            metric_value: Some(score),
            attributes: Some(attributes),
            is_interpolated: false,
        };

        observations.push(obs);
    }

    Ok(observations)
}

/// Fetches active macro BGP outages from CAIDA IODA API.
pub async fn fetch_caida_ioda(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_CAIDA_IODA_URL);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("CAIDA IODA HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "CAIDA IODA error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read CAIDA IODA body: {}", e)))?;

    let obs = parse_caida_ioda_json(&body)?;
    info!("Ingested {} active internet disruptions from CAIDA IODA.", obs.len());
    Ok(obs)
}
