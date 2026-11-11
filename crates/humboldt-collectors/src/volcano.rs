use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

pub const DEFAULT_USGS_VOLCANO_URL: &str = "https://volcanoes.usgs.gov/vsc/api/volcanoApi/geojson";

/// Parses USGS Volcano Hazards Program GeoJSON into normalized HUMBOLDT Observation records.
pub fn parse_usgs_volcano_geojson(json_str: &str) -> Result<Vec<Observation>> {
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

        let vnum = props.get("vnum")
            .or_else(|| props.get("v_num"))
            .or_else(|| props.get("id"))
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    Some(s.to_string())
                } else {
                    v.as_i64().map(|n| n.to_string())
                }
            })
            .unwrap_or_else(|| "volcano".to_string());

        let name = props.get("volcano_name")
            .or_else(|| props.get("v_name"))
            .or_else(|| props.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Volcano");

        let elev_m = props.get("elev_m")
            .or_else(|| props.get("elevation"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        let color_code = props.get("color_code")
            .or_else(|| props.get("color"))
            .and_then(|v| v.as_str())
            .unwrap_or("GREEN");

        let alert_level = props.get("alert_level")
            .or_else(|| props.get("alert"))
            .and_then(|v| v.as_str())
            .unwrap_or("NORMAL");

        let observatory = props.get("observatory")
            .or_else(|| props.get("obs"))
            .and_then(|v| v.as_str())
            .unwrap_or("USGS");

        // Numeric threat score: 0 (Normal/Green), 1 (Advisory/Yellow), 2 (Watch/Orange), 3 (Warning/Red)
        let threat_score = match color_code.to_uppercase().as_str() {
            "RED" => 3.0,
            "ORANGE" => 2.0,
            "YELLOW" => 1.0,
            _ => 0.0,
        };

        let attrs = serde_json::json!({
            "name": name,
            "color_code": color_code.to_uppercase(),
            "alert_level": alert_level.to_uppercase(),
            "observatory": observatory,
            "elevation_m": elev_m,
        });

        observations.push(Observation {
            id: None,
            timestamp: now,
            layer: LayerType::Volcano,
            provider: "usgs_vhp".to_string(),
            entity_id: Some(vnum),
            latitude: lat,
            longitude: lon,
            altitude_m: Some(elev_m),
            metric_name: Some("threat_level".to_string()),
            metric_value: Some(threat_score),
            attributes: Some(attrs),
            is_interpolated: false,
        });
    }

    Ok(observations)
}

/// Fetches active volcano statuses from USGS Volcano Hazards Program.
pub async fn fetch_usgs_volcanoes(client: &reqwest::Client, url_override: Option<&str>) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_USGS_VOLCANO_URL);
    info!("Fetching active volcano statuses from USGS VHP: {}", url);

    let resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("USGS VHP HTTP request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "USGS VHP responded with HTTP {}",
            resp.status()
        )));
    }

    let body = resp.text().await.map_err(|e| HumboldtError::Storage(format!("Failed to read USGS VHP body: {}", e)))?;
    parse_usgs_volcano_geojson(&body)
}
