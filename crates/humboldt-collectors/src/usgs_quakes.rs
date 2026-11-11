use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use serde::Deserialize;
use tracing::info;

pub const DEFAULT_USGS_URL: &str =
    "https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson";

#[derive(Debug, Deserialize)]
struct UsgsFeatureCollection {
    pub features: Vec<UsgsFeature>,
}

#[derive(Debug, Deserialize)]
struct UsgsFeature {
    pub id: Option<String>,
    pub properties: UsgsProperties,
    pub geometry: UsgsGeometry,
}

#[derive(Debug, Deserialize)]
struct UsgsProperties {
    pub mag: Option<f64>,
    pub place: Option<String>,
    pub time: Option<i64>,
    pub url: Option<String>,
    pub detail: Option<String>,
    pub status: Option<String>,
    pub tsunami: Option<i32>,
    pub sig: Option<i32>,
    #[serde(rename = "magType")]
    pub mag_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsgsGeometry {
    pub coordinates: Vec<f64>,
}

/// Parses raw USGS GeoJSON summary payload into normalized Observation records.
pub fn parse_usgs_geojson(json_str: &str) -> Result<Vec<Observation>> {
    let collection: UsgsFeatureCollection = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let mut observations = Vec::with_capacity(collection.features.len());

    for feature in collection.features {
        if feature.geometry.coordinates.len() < 2 {
            continue;
        }

        let lon = feature.geometry.coordinates[0];
        let lat = feature.geometry.coordinates[1];
        let depth_km = feature.geometry.coordinates.get(2).copied().unwrap_or(0.0);
        let altitude_m = -depth_km * 1000.0; // Depth in meters relative to surface

        let timestamp = feature.properties.time.unwrap_or(0);
        let magnitude = feature.properties.mag.unwrap_or(0.0);

        let attributes = serde_json::json!({
            "place": feature.properties.place,
            "url": feature.properties.url,
            "detail": feature.properties.detail,
            "status": feature.properties.status,
            "tsunami": feature.properties.tsunami,
            "sig": feature.properties.sig,
            "mag_type": feature.properties.mag_type,
        });

        let obs = Observation {
            id: None,
            timestamp,
            layer: LayerType::Quake,
            provider: "usgs_quakes".to_string(),
            entity_id: feature.id,
            latitude: Some(lat),
            longitude: Some(lon),
            altitude_m: Some(altitude_m),
            metric_name: Some("magnitude".to_string()),
            metric_value: Some(magnitude),
            attributes: Some(attributes),
            is_interpolated: false,
        };

        if obs.is_valid_geo() {
            observations.push(obs);
        }
    }

    Ok(observations)
}

/// Ingests live USGS earthquake summary feed using reqwest client.
pub async fn fetch_usgs_quakes(
    client: &reqwest::Client,
    url_override: Option<&str>,
) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_USGS_URL);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("USGS HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "USGS HTTP error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read USGS body: {}", e)))?;

    let quakes = parse_usgs_geojson(&body)?;
    info!("Fetched and parsed {} earthquakes from USGS.", quakes.len());
    Ok(quakes)
}
