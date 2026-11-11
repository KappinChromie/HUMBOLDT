use chrono::DateTime;
use humboldt_core::{HumboldtError, LayerType, Observation, Result};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Deserialize)]
pub struct AisStreamEnvelope {
    #[serde(rename = "MessageType")]
    pub message_type: Option<String>,
    #[serde(rename = "MetaData")]
    pub metadata: Option<AisMetadata>,
    #[serde(rename = "Message")]
    pub message: Option<AisInnerMessage>,
}

#[derive(Debug, Deserialize)]
pub struct AisMetadata {
    #[serde(rename = "MMSI")]
    pub mmsi: Option<i64>,
    #[serde(rename = "ShipName")]
    pub ship_name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub time_utc: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AisInnerMessage {
    #[serde(rename = "PositionReport")]
    pub position_report: Option<AisPositionReport>,
}

#[derive(Debug, Deserialize)]
pub struct AisPositionReport {
    #[serde(rename = "Cog")]
    pub cog: Option<f64>,
    #[serde(rename = "Sog")]
    pub sog: Option<f64>,
    #[serde(rename = "TrueHeading")]
    pub true_heading: Option<i32>,
    #[serde(rename = "NavigationalStatus")]
    pub nav_status: Option<i32>,
}

/// Parses an AISstream single message or array of messages into normalized Observation records.
pub fn parse_ais_json(json_str: &str) -> Result<Vec<Observation>> {
    // Attempt parsing as array or single envelope
    let envelopes: Vec<AisStreamEnvelope> = if json_str.trim().starts_with('[') {
        serde_json::from_str(json_str).map_err(HumboldtError::Serialization)?
    } else {
        let single: AisStreamEnvelope = serde_json::from_str(json_str)
            .map_err(HumboldtError::Serialization)?;
        vec![single]
    };

    let mut observations = Vec::with_capacity(envelopes.len());

    for env in envelopes {
        if let Some(meta) = env.metadata {
            if let (Some(lat), Some(lon), Some(mmsi)) = (meta.latitude, meta.longitude, meta.mmsi) {
                let timestamp = meta
                    .time_utc
                    .as_deref()
                    .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                    .map(|dt| dt.timestamp_millis())
                    .unwrap_or_else(|| {
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64
                    });

                let sog = env
                    .message
                    .as_ref()
                    .and_then(|m| m.position_report.as_ref())
                    .and_then(|p| p.sog);

                let cog = env
                    .message
                    .as_ref()
                    .and_then(|m| m.position_report.as_ref())
                    .and_then(|p| p.cog);

                let heading = env
                    .message
                    .as_ref()
                    .and_then(|m| m.position_report.as_ref())
                    .and_then(|p| p.true_heading);

                let ship_name = meta.ship_name.map(|s| s.trim().to_string());
                let attributes = serde_json::json!({
                    "ship_name": ship_name,
                    "mmsi": mmsi,
                    "cog_deg": cog,
                    "sog_knots": sog,
                    "true_heading": heading,
                });

                let obs = Observation {
                    id: None,
                    timestamp,
                    layer: LayerType::Ship,
                    provider: "aisstream".to_string(),
                    entity_id: Some(mmsi.to_string()),
                    latitude: Some(lat),
                    longitude: Some(lon),
                    altitude_m: Some(0.0), // Sea level
                    metric_name: Some("sog_knots".to_string()),
                    metric_value: sog,
                    attributes: Some(attributes),
                    is_interpolated: false,
                };

                if obs.is_valid_geo() {
                    observations.push(obs);
                }
            }
        }
    }

    Ok(observations)
}
