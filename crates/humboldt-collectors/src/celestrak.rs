use chrono::{DateTime, Utc};
use humboldt_core::{CANONICAL_USER_AGENT, HumboldtError, LayerType, Observation, Result};
use tracing::{error, info};

pub const DEFAULT_CELESTRAK_URL: &str =
    "https://celestrak.org/NORAD/elements/gp.php?GROUP=active&FORMAT=json";

// WGS84 Constants (km)
const WGS84_A: f64 = 6378.137;
const WGS84_F: f64 = 1.0 / 298.257223563;
const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F);
const WGS84_E2: f64 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_A * WGS84_A);
const WGS84_E_PRIME2: f64 = (WGS84_A * WGS84_A - WGS84_B * WGS84_B) / (WGS84_B * WGS84_B);

/// Propagates orbital elements to WGS84 Geodetic coordinates (lat, lon, altitude_m)
/// at the specified target time using local SGP4/SDP4 propagation.
pub fn propagate_satellite_wgs84(
    elements: &sgp4::Elements,
    target_time: DateTime<Utc>,
) -> Result<(f64, f64, f64)> {
    let constants = sgp4::Constants::from_elements(elements)
        .map_err(|e| HumboldtError::Storage(format!("SGP4 constants error: {}", e)))?;

    let epoch_naive = elements.datetime;
    let target_naive = target_time.naive_utc();
    let diff = target_naive.signed_duration_since(epoch_naive);
    let minutes_since_epoch = diff.num_milliseconds() as f64 / 60000.0;

    let prediction = constants
        .propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))
        .map_err(|e| HumboldtError::Storage(format!("SGP4 propagation error: {}", e)))?;

    let x_teme = prediction.position[0];
    let y_teme = prediction.position[1];
    let z_teme = prediction.position[2];

    // Compute Greenwich Mean Sidereal Time (GMST) for coordinate transform TEME -> ECEF
    let j2000_years = sgp4::julian_years_since_j2000(&target_naive);
    let gmst = sgp4::iau_epoch_to_sidereal_time(j2000_years);

    // Rotate TEME coordinates by GMST around Z axis to obtain ECEF (Earth-Centered Earth-Fixed)
    let x_ecef = x_teme * gmst.cos() + y_teme * gmst.sin();
    let y_ecef = -x_teme * gmst.sin() + y_teme * gmst.cos();
    let z_ecef = z_teme;

    // Bowring's geodetic transformation (ECEF km to WGS84 lat, lon, altitude meters)
    let p = (x_ecef * x_ecef + y_ecef * y_ecef).sqrt();
    let theta = (z_ecef * WGS84_A).atan2(p * WGS84_B);
    let sin_theta = theta.sin();
    let cos_theta = theta.cos();

    let lat_rad = (z_ecef + WGS84_E_PRIME2 * WGS84_B * sin_theta.powi(3))
        .atan2(p - WGS84_E2 * WGS84_A * cos_theta.powi(3));
    let lon_rad = y_ecef.atan2(x_ecef);

    let sin_lat = lat_rad.sin();
    let n = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    let alt_km = (p / lat_rad.cos().max(1e-10)) - n;

    let lat_deg = lat_rad.to_degrees();
    let lon_deg = lon_rad.to_degrees();
    let alt_m = alt_km * 1000.0;

    Ok((lat_deg, lon_deg, alt_m))
}

/// Parses CelesTrak GP JSON payload and propagates satellites to current target time.
pub fn parse_celestrak_json(
    json_str: &str,
    target_time: DateTime<Utc>,
) -> Result<Vec<Observation>> {
    let elements_vec: Vec<sgp4::Elements> = serde_json::from_str(json_str)
        .map_err(HumboldtError::Serialization)?;

    let mut observations = Vec::with_capacity(elements_vec.len());
    let now_ms = target_time.timestamp_millis();

    for elements in &elements_vec {
        match propagate_satellite_wgs84(elements, target_time) {
            Ok((lat, lon, alt_m)) => {
                let entity_id = elements.norad_id.to_string();
                let sat_name = elements.object_name.clone().unwrap_or_else(|| entity_id.clone());

                let attributes = serde_json::json!({
                    "name": sat_name,
                    "norad_cat_id": elements.norad_id,
                    "international_designator": elements.international_designator,
                    "epoch": elements.datetime.to_string(),
                    "inclination_deg": elements.inclination.to_degrees(),
                    "eccentricity": elements.eccentricity,
                    "mean_motion_rev_day": elements.mean_motion * (86400.0 / (2.0 * std::f64::consts::PI)),
                });

                let obs = Observation {
                    id: None,
                    timestamp: now_ms,
                    layer: LayerType::Satellite,
                    provider: "celestrak".to_string(),
                    entity_id: Some(entity_id),
                    latitude: Some(lat),
                    longitude: Some(lon),
                    altitude_m: Some(alt_m),
                    metric_name: Some("altitude_m".to_string()),
                    metric_value: Some(alt_m),
                    attributes: Some(attributes),
                    is_interpolated: false,
                };

                if obs.is_valid_geo() {
                    observations.push(obs);
                }
            }
            Err(err) => {
                error!(
                    "Failed to propagate satellite {:?}: {}",
                    elements.object_name, err
                );
            }
        }
    }

    Ok(observations)
}

/// Ingests active satellites from CelesTrak GP endpoint using reqwest client.
pub async fn fetch_celestrak_satellites(
    client: &reqwest::Client,
    url_override: Option<&str>,
    target_time: DateTime<Utc>,
) -> Result<Vec<Observation>> {
    let url = url_override.unwrap_or(DEFAULT_CELESTRAK_URL);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, CANONICAL_USER_AGENT)
        .send()
        .await
        .map_err(|e| HumboldtError::Storage(format!("CelesTrak HTTP request failed: {}", e)))?;

    if !response.status().is_success() {
        return Err(HumboldtError::Storage(format!(
            "CelesTrak HTTP error: status {}",
            response.status()
        )));
    }

    let body = response
        .text()
        .await
        .map_err(|e| HumboldtError::Storage(format!("Failed to read CelesTrak body: {}", e)))?;

    let satellites = parse_celestrak_json(&body, target_time)?;
    info!(
        "Fetched and propagated {} satellites from CelesTrak.",
        satellites.len()
    );
    Ok(satellites)
}
