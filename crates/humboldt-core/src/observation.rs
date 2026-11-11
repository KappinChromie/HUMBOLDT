use serde::{Deserialize, Serialize};

/// Supported observation telemetry layers per HUMBOLDT Master Specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    Aircraft,
    Ship,
    Satellite,
    Quake,
    Weather,
    Wildfire,
    SpaceWeather,
    Hydro,
    Buoy,
    PowerGrid,
    InternetHealth,
    Radar,
    AirQuality,
    Cyclone,
    Volcano,
    SubmarineCable,
    Radiation,
    Aurora,
    Disaster,
    News,
    Alert,
}

impl LayerType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LayerType::Aircraft => "aircraft",
            LayerType::Ship => "ship",
            LayerType::Satellite => "satellite",
            LayerType::Quake => "quake",
            LayerType::Weather => "weather",
            LayerType::Wildfire => "wildfire",
            LayerType::SpaceWeather => "space_weather",
            LayerType::Hydro => "hydro",
            LayerType::Buoy => "buoy",
            LayerType::PowerGrid => "power_grid",
            LayerType::InternetHealth => "internet_health",
            LayerType::Radar => "radar",
            LayerType::AirQuality => "air_quality",
            LayerType::Cyclone => "cyclone",
            LayerType::Volcano => "volcano",
            LayerType::SubmarineCable => "submarine_cable",
            LayerType::Radiation => "radiation",
            LayerType::Aurora => "aurora",
            LayerType::Disaster => "disaster",
            LayerType::News => "news",
            LayerType::Alert => "alert",
        }
    }
}

impl std::fmt::Display for LayerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Operational routing mode for provider ingest scheduler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingMode {
    Failover,
    Spread,
    Merge,
}

/// Core telemetry observation matching Architecture Spec Section 5.2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Unix epoch milliseconds (UTC)
    pub timestamp: i64,
    pub layer: LayerType,
    /// Provider slug (e.g. 'adsb_lol', 'usgs_quakes', 'celestrak')
    pub provider: String,
    /// Normalized ID (ICAO24 hex, MMSI, NORAD-ID, Station ID)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<String>,
    /// WGS84 Latitude (-90.0 to 90.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    /// WGS84 Longitude (-180.0 to 180.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    /// Altitude / Elevation in meters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_m: Option<f64>,
    /// Metric identifier (e.g. 'kp_index', 'temp_c', 'magnitude', 'gauge_height_m')
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_name: Option<String>,
    /// Numeric scalar reading for analytical graphs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metric_value: Option<f64>,
    /// Serialized JSON payload of layer-specific attributes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<serde_json::Value>,
    /// Flag indicating if coordinates/reading are dead-reckoned or estimated
    #[serde(default)]
    pub is_interpolated: bool,
}

impl Observation {
    /// Validates spatial coordinates if present.
    pub fn is_valid_geo(&self) -> bool {
        match (self.latitude, self.longitude) {
            (Some(lat), Some(lon)) => {
                (-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)
            }
            (None, None) => true,
            _ => false,
        }
    }
}
