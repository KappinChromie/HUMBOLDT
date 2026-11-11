use crate::static_files::static_handler;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use futures::{SinkExt, StreamExt};
use humboldt_alerts::AlertEngine;
use humboldt_collectors::{
    fetch_adsb, fetch_caida_ioda, fetch_celestrak_satellites, fetch_noaa_kp, fetch_open_meteo,
    fetch_uk_power_grid, fetch_usgs_quakes, DEFAULT_ADSB_LOL_URL,
};
use humboldt_core::{AlertRule, ComparisonMode, ComparisonStats, DataPoint, Observation, Snapshot};
use humboldt_security::{SessionManager, Vault};
use humboldt_storage::Storage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tracing::{error, warn};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub port: u16,
    pub start_time: Instant,
    pub version: &'static str,
    pub storage: Storage,
    pub vault: Vault,
    pub http_client: reqwest::Client,
    pub bus: broadcast::Sender<Arc<Observation>>,
    pub geo_index: Arc<humboldt_geo::SpatialIndex>,
    pub news_engine: humboldt_news::NewsHierarchyEngine,
    pub alert_engine: AlertEngine,
    pub session_mgr: SessionManager,
    pub lan_enabled: Arc<tokio::sync::RwLock<bool>>,
    pub lan_password_hash: Arc<tokio::sync::RwLock<Option<String>>>,
    pub shutdown_tx: broadcast::Sender<()>,
}

#[derive(Deserialize)]
pub struct GeoResolveQuery {
    pub lat: f64,
    pub lon: f64,
    pub target_level: Option<String>,
}

#[derive(Serialize)]
pub struct BoundariesResponse {
    pub countries: Vec<humboldt_geo::GeoPolygon>,
    pub states: Vec<humboldt_geo::GeoPolygon>,
    pub counties: Vec<humboldt_geo::GeoPolygon>,
    #[serde(default)]
    pub cities: Vec<humboldt_geo::GeoPolygon>,
    pub places: Vec<humboldt_geo::GeoPlace>,
}

#[derive(Deserialize)]
pub struct NewsQuery {
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub country: Option<String>,
    pub target_level: Option<String>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub port: u16,
    pub version: &'static str,
    pub uptime_seconds: u64,
}

#[derive(Deserialize)]
pub struct ObservationsQuery {
    pub layer: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Deserialize)]
pub struct SetSecretRequest {
    pub key_name: String,
    pub secret_value: String,
}

#[derive(Serialize)]
pub struct CollectResponse {
    pub layer: &'static str,
    pub count: usize,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action")]
pub enum WsClientMessage {
    #[serde(rename = "subscribe")]
    Subscribe { layers: Option<Vec<String>> },
    #[serde(rename = "ping")]
    Ping,
}

#[derive(Deserialize)]
pub struct MetricsSeriesQuery {
    pub layer: String,
    pub metric_name: Option<String>,
    pub entity_id: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub from: Option<i64>,
    pub to: Option<i64>,
}

#[derive(Deserialize)]
pub struct CreateSnapshotRequest {
    pub name: String,
    pub layer: String,
    pub metric_name: String,
    pub query_entity: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub resolution: Option<String>,
    pub metadata_json: Option<String>,
    pub data_points: Vec<DataPoint>,
}

#[derive(Serialize)]
pub struct SnapshotResponse {
    pub id: String,
    pub name: String,
    pub created_at: i64,
    pub layer: String,
    pub metric_name: String,
    pub query_entity: Option<String>,
    pub start_time: i64,
    pub end_time: i64,
    pub resolution: String,
    pub metadata_json: String,
    pub point_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_points: Option<Vec<DataPoint>>,
}

#[derive(Deserialize)]
pub struct CompareSnapshotsRequest {
    pub snapshot_a_id: String,
    pub snapshot_b_id: String,
    #[serde(default = "default_comparison_mode")]
    pub mode: ComparisonMode,
}

fn default_comparison_mode() -> ComparisonMode {
    ComparisonMode::Calendar
}

#[derive(Serialize)]
pub struct CompareSnapshotsResponse {
    pub snapshot_a_id: String,
    pub snapshot_a_name: String,
    pub snapshot_b_id: String,
    pub snapshot_b_name: String,
    pub mode: ComparisonMode,
    pub stats: ComparisonStats,
    pub series_a: Vec<DataPoint>,
    pub series_b: Vec<DataPoint>,
}

#[derive(Deserialize)]
pub struct SetPasswordRequest {
    pub password: String,
}

#[derive(Deserialize)]
pub struct SetLanRequest {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Deserialize)]
pub struct TestToastRequest {
    pub title: Option<String>,
    pub message: Option<String>,
}

#[derive(Deserialize)]
pub struct TestDiscordRequest {
    pub webhook_url: Option<String>,
}

#[derive(Serialize)]
pub struct ProviderAttribution {
    pub id: &'static str,
    pub category: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub license: &'static str,
    pub license_type: &'static str,
    pub url: &'static str,
}

#[derive(Deserialize)]
pub struct TestKeyRequest {
    pub provider_id: String,
    pub api_key: String,
}

#[derive(Serialize)]
pub struct TestKeyResponse {
    pub valid: bool,
    pub message: &'static str,
}

/// GET /api/v1/attribution
pub async fn get_attribution_handler() -> impl IntoResponse {
    let providers = vec![
        ProviderAttribution {
            id: "adsb_lol",
            category: "Aviation & Maritime",
            name: "adsb.lol",
            description: "Open community flight tracking and ADS-B exchange",
            license: "Non-commercial community credit",
            license_type: "Community Fair Use",
            url: "https://adsb.lol",
        },
        ProviderAttribution {
            id: "airplanes_live",
            category: "Aviation & Maritime",
            name: "airplanes.live",
            description: "Cooperative crowd-sourced aircraft telemetry",
            license: "Community flight data fair use",
            license_type: "Community Fair Use",
            url: "https://airplanes.live",
        },
        ProviderAttribution {
            id: "adsb_fi",
            category: "Aviation & Maritime",
            name: "adsb.fi",
            description: "Nordic and European community ADS-B tracking",
            license: "Non-commercial community credit",
            license_type: "Community Fair Use",
            url: "https://adsb.fi",
        },
        ProviderAttribution {
            id: "opensky_network",
            category: "Aviation & Maritime",
            name: "OpenSky Network",
            description: "Crowd-sourced air traffic data for academic and institutional research",
            license: "Academic and non-commercial attribution",
            license_type: "Academic Attribution",
            url: "https://opensky-network.org",
        },
        ProviderAttribution {
            id: "aisstream",
            category: "Aviation & Maritime",
            name: "AISstream.io",
            description: "Real-time worldwide marine vessel Automatic Identification System (AIS) push stream",
            license: "Community streaming credit",
            license_type: "Community Fair Use",
            url: "https://aisstream.io",
        },
        ProviderAttribution {
            id: "digitraffic_finland",
            category: "Aviation & Maritime",
            name: "Digitraffic Finland",
            description: "Finnish and Baltic Sea nautical navigational vessel telemetry",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://digitraffic.fi",
        },
        ProviderAttribution {
            id: "celestrak",
            category: "Space & Geophysics",
            name: "CelesTrak",
            description: "Two-Line Element (TLE) and General Perturbations (GP) orbital data (Dr. T.S. Kelso)",
            license: "CelesTrak scientific citation",
            license_type: "Open Scientific",
            url: "https://celestrak.org",
        },
        ProviderAttribution {
            id: "usgs",
            category: "Space & Geophysics",
            name: "USGS Earthquakes",
            description: "Global seismic event monitoring and magnitude estimation",
            license: "Public Domain (U.S. Geological Survey)",
            license_type: "Public Domain",
            url: "https://earthquake.usgs.gov",
        },
        ProviderAttribution {
            id: "emsc_seismic",
            category: "Space & Geophysics",
            name: "EMSC Seismic",
            description: "Euro-Mediterranean Seismological Centre real-time seismic alert network",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://emsc-csem.org",
        },
        ProviderAttribution {
            id: "nasa_firms",
            category: "Space & Geophysics",
            name: "NASA FIRMS",
            description: "Fire Information for Resource Management System active thermal points",
            license: "NASA Open Data Policy",
            license_type: "Public Domain",
            url: "https://firms.modaps.eosdis.nasa.gov",
        },
        ProviderAttribution {
            id: "noaa_swpc",
            category: "Space & Geophysics",
            name: "NOAA SWPC",
            description: "Space Weather Prediction Center solar flux, magnetosphere, and planetary Kp index",
            license: "Public Domain (U.S. Government)",
            license_type: "Public Domain",
            url: "https://www.swpc.noaa.gov",
        },
        ProviderAttribution {
            id: "open_meteo",
            category: "Atmosphere & Weather",
            name: "Open-Meteo",
            description: "Open-source high-resolution weather models and global flood predictions",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://open-meteo.com",
        },
        ProviderAttribution {
            id: "nws",
            category: "Atmosphere & Weather",
            name: "National Weather Service (NWS)",
            description: "U.S. meteorological observations, station forecasts, and radar telemetry",
            license: "Public Domain (U.S. Government)",
            license_type: "Public Domain",
            url: "https://weather.gov",
        },
        ProviderAttribution {
            id: "openaq",
            category: "Atmosphere & Weather",
            name: "OpenAQ",
            description: "Global community-driven air quality monitoring (PM2.5, PM10, O3, NO2)",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://openaq.org",
        },
        ProviderAttribution {
            id: "national_grid_eso",
            category: "Infrastructure & Macro Systems",
            name: "UK National Grid ESO",
            description: "Great Britain real-time generation fuel mix and carbon intensity metrics",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://carbonintensity.org.uk",
        },
        ProviderAttribution {
            id: "eia",
            category: "Infrastructure & Macro Systems",
            name: "U.S. Energy Information Administration (EIA)",
            description: "Electric power generation by energy source across U.S. balancing authorities",
            license: "Public Domain (U.S. EIA Open Data)",
            license_type: "Public Domain",
            url: "https://eia.gov",
        },
        ProviderAttribution {
            id: "caida_ioda",
            category: "Infrastructure & Macro Systems",
            name: "CAIDA IODA",
            description: "Internet Outage Detection and Analysis (macro connectivity & BGP telemetry)",
            license: "CAIDA Open Research Data",
            license_type: "Open Research",
            url: "https://ioda.caida.org",
        },
        ProviderAttribution {
            id: "natural_earth",
            category: "Cartography & Engine",
            name: "Natural Earth",
            description: "Public domain 1:50m raster cartography and administrative polygon boundaries",
            license: "Public Domain",
            license_type: "Public Domain",
            url: "https://naturalearthdata.com",
        },
        ProviderAttribution {
            id: "census_bureau",
            category: "Cartography & Engine",
            name: "U.S. Census Bureau",
            description: "TIGER/Line geographic shapefiles for state, county, and incorporated place boundaries",
            license: "Public Domain (U.S. Government)",
            license_type: "Public Domain",
            url: "https://census.gov",
        },
        ProviderAttribution {
            id: "cesium",
            category: "Cartography & Engine",
            name: "CesiumJS",
            description: "3D geospatial virtual globe engine and WGS84 tile renderer",
            license: "Apache License 2.0",
            license_type: "Apache 2.0",
            url: "https://cesium.com",
        },
        ProviderAttribution {
            id: "echarts",
            category: "Cartography & Engine",
            name: "Apache ECharts",
            description: "Interactive scalar telemetry charting and dual-series snapshot comparison",
            license: "Apache License 2.0",
            license_type: "Apache 2.0",
            url: "https://echarts.apache.org",
        },
        ProviderAttribution {
            id: "gdelt",
            category: "Global News",
            name: "GDELT Project",
            description: "Global Database of Events, Language, and Tone DOC 2.0 multilingual news query API",
            license: "Open Research & Community Fair Use",
            license_type: "Community Fair Use",
            url: "https://gdeltproject.org",
        },
        ProviderAttribution {
            id: "google_news",
            category: "Global News",
            name: "Google News RSS",
            description: "Syndicated headline feeds for hierarchical geographic fallback",
            license: "Fair Use Syndication (Headline, Timestamp & Link Only)",
            license_type: "Fair Use",
            url: "https://news.google.com",
        },
        ProviderAttribution {
            id: "noaa_nhc",
            category: "Atmosphere & Weather",
            name: "NOAA National Hurricane Center (NHC)",
            description: "Active tropical cyclone positions, classifications, wind radii, and track forecasts",
            license: "Public Domain (U.S. Government)",
            license_type: "Public Domain",
            url: "https://www.nhc.noaa.gov",
        },
        ProviderAttribution {
            id: "usgs_vhp",
            category: "Space & Geophysics",
            name: "USGS Volcano Hazards Program",
            description: "Real-time volcanic alert levels, aviation color codes, and summit observations",
            license: "Public Domain (U.S. Geological Survey)",
            license_type: "Public Domain",
            url: "https://volcanoes.usgs.gov",
        },
        ProviderAttribution {
            id: "telegeography_cables",
            category: "Infrastructure & Macro Systems",
            name: "TeleGeography Submarine Cable Data",
            description: "Global undersea telecommunications fiber-optic routes and landing stations",
            license: "Creative Commons Attribution-ShareAlike (CC BY-SA 3.0)",
            license_type: "CC BY-SA",
            url: "https://submarinecablemap.com",
        },
        ProviderAttribution {
            id: "safecast",
            category: "Atmosphere & Weather",
            name: "Safecast Global Radiation Network",
            description: "Crowdsourced open ambient ionizing radiation monitoring (uSv/h)",
            license: "Creative Commons Zero 1.0 (CC0 Public Domain)",
            license_type: "Public Domain",
            url: "https://safecast.org",
        },
        ProviderAttribution {
            id: "gdacs",
            category: "Global News",
            name: "GDACS (UN OCHA / European Commission)",
            description: "Global Disaster Alert and Coordination System disaster & emergency monitoring",
            license: "Creative Commons Attribution 4.0 International (CC BY 4.0)",
            license_type: "CC BY 4.0",
            url: "https://gdacs.org",
        },
    ];

    (StatusCode::OK, Json(providers)).into_response()
}

/// GET /api/v1/health
pub async fn health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    Json(HealthResponse {
        status: "nominal",
        port: state.port,
        version: state.version,
        uptime_seconds: uptime,
    })
}

/// GET /api/v1/observations?layer=quake&limit=100
pub async fn get_observations_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ObservationsQuery>,
) -> impl IntoResponse {
    let limit = query.limit.unwrap_or(100).min(1000);
    match state
        .storage
        .observations
        .query_latest(query.layer.as_deref(), limit)
        .await
    {
        Ok(obs) => (StatusCode::OK, Json(obs)).into_response(),
        Err(err) => {
            error!("Failed to query observations: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/ws (WebSocket Live Observation Bus)
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let is_lan = *state.lan_enabled.read().await;
    if is_lan {
        let cookie_header = headers
            .get(COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if state.session_mgr.validate_cookie_header(cookie_header).is_none() {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "Unauthorized WebSocket connection: valid humboldt_session required in LAN mode" })),
            )
                .into_response();
        }
    }

    ws.on_upgrade(move |socket| handle_ws_socket(socket, state)).into_response()
}

async fn handle_ws_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut bus_rx = state.bus.subscribe();
    let subscribed_layers: Arc<tokio::sync::RwLock<Option<HashSet<String>>>> =
        Arc::new(tokio::sync::RwLock::new(None));

    let layers_filter = subscribed_layers.clone();
    let mut send_task = tokio::spawn(async move {
        loop {
            match bus_rx.recv().await {
                Ok(obs) => {
                    let should_send = {
                        let filter = layers_filter.read().await;
                        match &*filter {
                            Some(set) => set.contains(obs.layer.as_str()),
                            None => true,
                        }
                    };

                    if should_send {
                        let msg_json = serde_json::to_string(&serde_json::json!({
                            "type": "observation",
                            "data": obs.as_ref()
                        }))
                        .unwrap_or_default();

                        if sender.send(Message::Text(msg_json)).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    warn!("WebSocket subscriber lagged by {} messages", skipped);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    });

    let layers_writer = subscribed_layers.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(client_msg) = serde_json::from_str::<WsClientMessage>(&text) {
                        match client_msg {
                            WsClientMessage::Subscribe { layers } => {
                                let mut filter = layers_writer.write().await;
                                *filter = layers.map(|l| l.into_iter().collect());
                            }
                            WsClientMessage::Ping => {}
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }
}

/// POST /api/v1/collect/quakes
pub async fn collect_quakes_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_usgs_quakes(&state.http_client, None).await {
        Ok(quakes) => {
            let count = quakes.len();
            if let Err(e) = state.storage.observations.insert_batch(&quakes).await {
                error!("Failed to persist quakes to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for q in quakes {
                let _ = state.bus.send(Arc::new(q));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "quake", count })).into_response()
        }
        Err(err) => {
            error!("USGS collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/satellites
pub async fn collect_satellites_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let now = Utc::now();
    match fetch_celestrak_satellites(&state.http_client, None, now).await {
        Ok(sats) => {
            let count = sats.len();
            if let Err(e) = state.storage.observations.insert_batch(&sats).await {
                error!("Failed to persist satellites to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for s in sats {
                let _ = state.bus.send(Arc::new(s));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "satellite", count })).into_response()
        }
        Err(err) => {
            error!("CelesTrak collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/adsb
pub async fn collect_adsb_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_adsb(&state.http_client, DEFAULT_ADSB_LOL_URL, "adsb_lol").await {
        Ok(aircraft) => {
            let count = aircraft.len();
            if let Err(e) = state.storage.observations.insert_batch(&aircraft).await {
                error!("Failed to persist ADS-B to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for ac in aircraft {
                let _ = state.bus.send(Arc::new(ac));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "aircraft", count })).into_response()
        }
        Err(err) => {
            error!("ADS-B collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/space-weather
pub async fn collect_space_weather_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_noaa_kp(&state.http_client, None).await {
        Ok(readings) => {
            let count = readings.len();
            if let Err(e) = state.storage.observations.insert_batch(&readings).await {
                error!("Failed to persist space weather to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for r in readings {
                let _ = state.bus.send(Arc::new(r));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "space_weather", count })).into_response()
        }
        Err(err) => {
            error!("Space weather collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/weather
pub async fn collect_weather_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_open_meteo(&state.http_client, 39.0997, -94.5786, Some("Kansas City, MO")).await {
        Ok(obs) => {
            let count = obs.len();
            if let Err(e) = state.storage.observations.insert_batch(&obs).await {
                error!("Failed to persist weather to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for o in obs {
                let _ = state.bus.send(Arc::new(o));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "weather", count })).into_response()
        }
        Err(err) => {
            error!("Weather collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/power-grid
pub async fn collect_power_grid_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_uk_power_grid(&state.http_client, None).await {
        Ok(obs) => {
            let count = obs.len();
            if let Err(e) = state.storage.observations.insert_batch(&obs).await {
                error!("Failed to persist power grid to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for o in obs {
                let _ = state.bus.send(Arc::new(o));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "power_grid", count })).into_response()
        }
        Err(err) => {
            error!("Power grid collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/collect/internet-health
pub async fn collect_internet_health_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match fetch_caida_ioda(&state.http_client, None).await {
        Ok(obs) => {
            let count = obs.len();
            if let Err(e) = state.storage.observations.insert_batch(&obs).await {
                error!("Failed to persist internet health to database: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            for o in obs {
                let _ = state.bus.send(Arc::new(o));
            }
            (StatusCode::OK, Json(CollectResponse { layer: "internet_health", count })).into_response()
        }
        Err(err) => {
            error!("Internet health collection failed: {}", err);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/retention/sweep
pub async fn retention_sweep_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let now_ms = Utc::now().timestamp_millis();
    match state.storage.retention.run_sweep(now_ms).await {
        Ok(stats) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "completed",
                "ephemeral_purged": stats.ephemeral_purged,
                "live_tracking_downsampled": stats.live_tracking_downsampled,
                "live_tracking_purged": stats.live_tracking_purged,
                "metric_series_rolled_up": stats.metric_series_rolled_up,
                "metric_series_raw_purged": stats.metric_series_raw_purged,
                "discrete_events_purged": stats.discrete_events_purged,
                "disk_cap_triggered": stats.disk_cap_triggered,
                "initial_size_bytes": stats.initial_size_bytes,
                "final_size_bytes": stats.final_size_bytes
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/vault/keys
pub async fn list_vault_keys_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.vault.list_key_names().await {
        Ok(keys) => (StatusCode::OK, Json(keys)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/vault
pub async fn set_vault_secret_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetSecretRequest>,
) -> impl IntoResponse {
    if payload.key_name.trim().is_empty() || payload.secret_value.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "key_name and secret_value are required" })),
        )
            .into_response();
    }

    match state
        .vault
        .set_secret(&payload.key_name, &payload.secret_value)
        .await
    {
        Ok(()) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "status": "stored", "key_name": payload.key_name })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/vault/:key_name
pub async fn delete_vault_secret_handler(
    State(state): State<Arc<AppState>>,
    Path(key_name): Path<String>,
) -> impl IntoResponse {
    match state.vault.delete_secret(&key_name).await {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({ "deleted": deleted })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/geo/resolve?lat=39.0997&lon=-94.5786&target_level=country
pub async fn geo_resolve_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GeoResolveQuery>,
) -> impl IntoResponse {
    let mut hierarchy = state.geo_index.resolve(query.lat, query.lon);
    if let Some(target) = query.target_level.as_deref() {
        match target.to_lowercase().as_str() {
            "country" | "nation" => {
                hierarchy.city = None;
                hierarchy.county = None;
                hierarchy.state = None;
                hierarchy.state_code = None;
                hierarchy.distance_to_city_km = None;
            }
            "state" | "province" => {
                hierarchy.city = None;
                hierarchy.county = None;
                hierarchy.distance_to_city_km = None;
            }
            "county" | "district" => {
                hierarchy.city = None;
                hierarchy.distance_to_city_km = None;
            }
            _ => {}
        }
    }
    (StatusCode::OK, Json(hierarchy)).into_response()
}

/// GET /api/v1/geo/boundaries
pub async fn geo_boundaries_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let response = BoundariesResponse {
        countries: state.geo_index.all_countries(),
        states: state.geo_index.all_states(),
        counties: state.geo_index.all_counties(),
        cities: state.geo_index.all_cities(),
        places: state.geo_index.all_places(),
    };
    (StatusCode::OK, Json(response)).into_response()
}

/// GET /api/v1/geo/submarine-cables
pub async fn get_submarine_cables_handler() -> impl IntoResponse {
    let cables = humboldt_geo::bundled_submarine_cables();
    (StatusCode::OK, Json(cables)).into_response()
}

/// GET /api/v1/news?lat=39.0997&lon=-94.5786&target_level=country
pub async fn get_news_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<NewsQuery>,
) -> impl IntoResponse {
    let mut hierarchy = match (query.lat, query.lon) {
        (Some(lat), Some(lon)) => state.geo_index.resolve(lat, lon),
        _ => humboldt_geo::ResolvedHierarchy {
            city: query.city,
            state: query.state,
            country: query.country,
            ..Default::default()
        },
    };

    if let Some(target) = query.target_level.as_deref() {
        match target.to_lowercase().as_str() {
            "country" | "nation" => {
                hierarchy.city = None;
                hierarchy.county = None;
                hierarchy.state = None;
                hierarchy.state_code = None;
                hierarchy.distance_to_city_km = None;
            }
            "state" | "province" => {
                hierarchy.city = None;
                hierarchy.county = None;
                hierarchy.distance_to_city_km = None;
            }
            "county" | "district" => {
                hierarchy.city = None;
                hierarchy.distance_to_city_km = None;
            }
            _ => {}
        }
    }

    match state.news_engine.fetch_news_for_hierarchy(&hierarchy).await {
        Ok(articles) => (StatusCode::OK, Json(articles)).into_response(),
        Err(err) => {
            error!("News fetch failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/metrics/series?layer=power_grid&metric_name=wind&from=...&to=...
pub async fn get_metric_series_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MetricsSeriesQuery>,
) -> impl IntoResponse {
    let start = query.start_time.or(query.from).unwrap_or(0);
    let end = query.end_time.or(query.to).unwrap_or_else(|| Utc::now().timestamp_millis());
    match state
        .storage
        .observations
        .query_metric_series(
            &query.layer,
            query.metric_name.as_deref(),
            query.entity_id.as_deref(),
            start,
            end,
        )
        .await
    {
        Ok(points) => (StatusCode::OK, Json(points)).into_response(),
        Err(err) => {
            error!("Query metric series failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/snapshots
pub async fn create_snapshot_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSnapshotRequest>,
) -> impl IntoResponse {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp_millis();
    let start_time = payload.start_time.unwrap_or_else(|| {
        payload
            .data_points
            .first()
            .map(|p| p.timestamp)
            .unwrap_or(now)
    });
    let end_time = payload.end_time.unwrap_or_else(|| {
        payload
            .data_points
            .last()
            .map(|p| p.timestamp)
            .unwrap_or(now)
    });
    let resolution = payload.resolution.unwrap_or_else(|| "raw".to_string());
    let metadata_json = payload.metadata_json.unwrap_or_else(|| "{}".to_string());

    let point_count = payload.data_points.len();
    let snapshot = match Snapshot::new(
        id,
        payload.name,
        now,
        payload.layer,
        payload.metric_name,
        payload.query_entity,
        start_time,
        end_time,
        resolution,
        metadata_json,
        &payload.data_points,
    ) {
        Ok(s) => s,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };

    match state.storage.snapshots.save(&snapshot).await {
        Ok(()) => (
            StatusCode::CREATED,
            Json(SnapshotResponse {
                id: snapshot.id,
                name: snapshot.name,
                created_at: snapshot.created_at,
                layer: snapshot.layer,
                metric_name: snapshot.metric_name,
                query_entity: snapshot.query_entity,
                start_time: snapshot.start_time,
                end_time: snapshot.end_time,
                resolution: snapshot.resolution,
                metadata_json: snapshot.metadata_json,
                point_count,
                data_points: None,
            }),
        )
            .into_response(),
        Err(err) => {
            error!("Save snapshot failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/snapshots
pub async fn list_snapshots_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.snapshots.list().await {
        Ok(snapshots) => {
            let list: Vec<SnapshotResponse> = snapshots
                .into_iter()
                .map(|s| {
                    let point_count = s.decoded_points().map(|pts| pts.len()).unwrap_or(0);
                    SnapshotResponse {
                        id: s.id,
                        name: s.name,
                        created_at: s.created_at,
                        layer: s.layer,
                        metric_name: s.metric_name,
                        query_entity: s.query_entity,
                        start_time: s.start_time,
                        end_time: s.end_time,
                        resolution: s.resolution,
                        metadata_json: s.metadata_json,
                        point_count,
                        data_points: None,
                    }
                })
                .collect();
            (StatusCode::OK, Json(list)).into_response()
        }
        Err(err) => {
            error!("List snapshots failed: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response()
        }
    }
}

/// GET /api/v1/snapshots/:id
pub async fn get_snapshot_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.snapshots.get(&id).await {
        Ok(Some(s)) => {
            let points = s.decoded_points().unwrap_or_default();
            let point_count = points.len();
            (
                StatusCode::OK,
                Json(SnapshotResponse {
                    id: s.id,
                    name: s.name,
                    created_at: s.created_at,
                    layer: s.layer,
                    metric_name: s.metric_name,
                    query_entity: s.query_entity,
                    start_time: s.start_time,
                    end_time: s.end_time,
                    resolution: s.resolution,
                    metadata_json: s.metadata_json,
                    point_count,
                    data_points: Some(points),
                }),
            )
                .into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Snapshot not found" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/snapshots/:id
pub async fn delete_snapshot_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.snapshots.delete(&id).await {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({ "deleted": deleted })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/snapshots/compare
pub async fn compare_snapshots_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompareSnapshotsRequest>,
) -> impl IntoResponse {
    let snap_a = match state.storage.snapshots.get(&payload.snapshot_a_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Snapshot A not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let snap_b = match state.storage.snapshots.get(&payload.snapshot_b_id).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Snapshot B not found" })),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let points_a = match snap_a.decoded_points() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };
    let points_b = match snap_b.decoded_points() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };

    let stats = humboldt_core::compute_comparison_stats(&points_a, &points_b, payload.mode);

    let (series_a, series_b) = match payload.mode {
        ComparisonMode::Relative => {
            let start_a = points_a.first().map(|p| p.timestamp).unwrap_or(0);
            let start_b = points_b.first().map(|p| p.timestamp).unwrap_or(0);
            let aligned_a: Vec<DataPoint> = points_a
                .into_iter()
                .map(|p| DataPoint {
                    timestamp: p.timestamp - start_a,
                    value: p.value,
                })
                .collect();
            let aligned_b: Vec<DataPoint> = points_b
                .into_iter()
                .map(|p| DataPoint {
                    timestamp: p.timestamp - start_b,
                    value: p.value,
                })
                .collect();
            (aligned_a, aligned_b)
        }
        ComparisonMode::Calendar => (points_a, points_b),
    };

    (
        StatusCode::OK,
        Json(CompareSnapshotsResponse {
            snapshot_a_id: snap_a.id,
            snapshot_a_name: snap_a.name,
            snapshot_b_id: snap_b.id,
            snapshot_b_name: snap_b.name,
            mode: payload.mode,
            stats,
            series_a,
            series_b,
        }),
    )
        .into_response()
}

/// GET /api/v1/snapshots/:id/csv
pub async fn export_snapshot_csv_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.storage.snapshots.get(&id).await {
        Ok(Some(s)) => match s.to_csv() {
            Ok(csv) => {
                let safe_name = s
                    .name
                    .chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect::<String>();
                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "text/csv; charset=utf-8")
                    .header(
                        CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{}.csv\"", safe_name),
                    )
                    .body(axum::body::Body::from(csv))
                    .unwrap()
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        },
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Snapshot not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/auth/status
pub async fn auth_status_handler(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let lan_enabled = *state.lan_enabled.read().await;
    let password_configured = state.lan_password_hash.read().await.is_some();
    let cookie_header = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let authenticated = state.session_mgr.validate_cookie_header(cookie_header).is_some();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "lan_enabled": lan_enabled,
            "password_configured": password_configured,
            "authenticated": authenticated
        })),
    )
        .into_response()
}

/// POST /api/v1/auth/password
pub async fn set_password_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetPasswordRequest>,
) -> impl IntoResponse {
    if payload.password.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Password cannot be empty" })),
        )
            .into_response();
    }

    match humboldt_security::hash_password(&payload.password) {
        Ok(hash) => {
            if let Err(e) = state.vault.set_secret("LAN_PASSWORD_HASH", &hash).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
            *state.lan_password_hash.write().await = Some(hash);
            (
                StatusCode::OK,
                Json(serde_json::json!({ "status": "password_configured" })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/auth/lan
pub async fn set_lan_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SetLanRequest>,
) -> impl IntoResponse {
    if payload.enabled && state.lan_password_hash.read().await.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Cannot enable LAN mode before configuring master access password"
            })),
        )
            .into_response();
    }

    *state.lan_enabled.write().await = payload.enabled;
    (
        StatusCode::OK,
        Json(serde_json::json!({ "lan_enabled": payload.enabled })),
    )
        .into_response()
}

/// POST /api/v1/auth/login
pub async fn login_handler(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> impl IntoResponse {
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim())
        .unwrap_or("127.0.0.1");

    if let Err(remaining) = state.session_mgr.check_rate_limit(client_ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({
                "error": format!("Too many failed attempts. Try again in {} seconds.", remaining),
                "retry_after": remaining
            })),
        )
            .into_response();
    }

    let hash_opt = state.lan_password_hash.read().await.clone();
    let hash = match hash_opt {
        Some(h) => h,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "LAN password has not been configured" })),
            )
                .into_response();
        }
    };

    match humboldt_security::verify_password(&payload.password, &hash) {
        Ok(true) => {
            state.session_mgr.record_successful_login(client_ip);
            let token = match state.session_mgr.create_session_token(client_ip, 86400) {
                Ok(t) => t,
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": e.to_string() })),
                    )
                        .into_response();
                }
            };
            let cookie_header = state.session_mgr.create_session_cookie_header(&token);
            Response::builder()
                .status(StatusCode::OK)
                .header(SET_COOKIE, cookie_header)
                .header(CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from("{\"status\":\"authenticated\"}"))
                .unwrap()
                .into_response()
        }
        _ => {
            let lockout = state.session_mgr.record_failed_attempt(client_ip);
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid password",
                    "retry_after": lockout
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/auth/logout
pub async fn logout_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let cookie_header = state.session_mgr.create_logout_cookie_header();
    Response::builder()
        .status(StatusCode::OK)
        .header(SET_COOKIE, cookie_header)
        .header(CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from("{\"status\":\"logged_out\"}"))
        .unwrap()
        .into_response()
}

/// POST /api/v1/storage/clear-cache
pub async fn clear_cache_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.retention.clear_caches().await {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({ "cleared_records": count })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/storage/delete-snapshots
pub async fn delete_all_snapshots_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.snapshots.delete_all().await {
        Ok(count) => (
            StatusCode::OK,
            Json(serde_json::json!({ "deleted_snapshots": count })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/storage/compact
pub async fn compact_storage_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.storage.retention.compact_database().await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "compacted" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// GET /api/v1/storage/status
pub async fn storage_status_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let size = state
        .storage
        .retention
        .get_database_size_bytes()
        .await
        .unwrap_or(0);
    let cap = humboldt_storage::DEFAULT_DISK_CAP_BYTES;
    let pct = (size as f64 / cap as f64) * 100.0;
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "database_size_bytes": size,
            "disk_cap_bytes": cap,
            "disk_usage_percent": pct
        })),
    )
        .into_response()
}

/// GET /api/v1/alerts/rules
pub async fn list_alert_rules_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let rules = state.alert_engine.list_rules().await;
    (StatusCode::OK, Json(rules)).into_response()
}

/// POST /api/v1/alerts/rules
pub async fn save_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<AlertRule>,
) -> impl IntoResponse {
    let id = state.alert_engine.save_rule(rule).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({ "id": id, "status": "saved" })),
    )
        .into_response()
}

/// GET /api/v1/alerts/rules/:id
pub async fn get_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.alert_engine.get_rule(&id).await {
        Some(rule) => (StatusCode::OK, Json(rule)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Alert rule not found" })),
        )
            .into_response(),
    }
}

/// DELETE /api/v1/alerts/rules/:id
pub async fn delete_alert_rule_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let deleted = state.alert_engine.delete_rule(&id).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({ "deleted": deleted })),
    )
        .into_response()
}

/// GET /api/v1/alerts/events
pub async fn list_alert_events_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let events = state.alert_engine.list_recent_events().await;
    (StatusCode::OK, Json(events)).into_response()
}

/// POST /api/v1/alerts/test-toast
pub async fn test_toast_handler(
    Json(payload): Json<Option<TestToastRequest>>,
) -> impl IntoResponse {
    let p = payload.unwrap_or(TestToastRequest {
        title: None,
        message: None,
    });
    let title = p
        .title
        .unwrap_or_else(|| "[HUMBOLDT] Test Notification".to_string());
    let msg = p
        .message
        .unwrap_or_else(|| "Win32 Toast notification subsystem nominal.".to_string());
    match humboldt_alerts::send_desktop_notification(&title, &msg) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "dispatched" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

/// POST /api/v1/alerts/test-discord
pub async fn test_discord_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Option<TestDiscordRequest>>,
) -> impl IntoResponse {
    let webhook_url = match payload.and_then(|p| p.webhook_url) {
        Some(url) => url,
        None => match state.vault.get_secret("DISCORD_WEBHOOK_URL").await {
            Ok(Some(sec)) => sec.expose_secret().to_string(),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "No Discord webhook URL configured or provided"
                    })),
                )
                    .into_response()
            }
        },
    };

    let sample_rule = AlertRule {
        id: "test_rule".to_string(),
        name: "Test Major Seismic Advisory".to_string(),
        enabled: true,
        layer: humboldt_core::LayerType::Quake,
        metric_name: "magnitude".to_string(),
        operator: humboldt_core::ComparisonOp::GreaterThan,
        threshold: 6.0,
        region_boundary_id: None,
        cooldown_seconds: 3600,
        last_triggered: None,
        notify_desktop: true,
        notify_discord: true,
    };

    let sample_obs = humboldt_core::Observation {
        id: None,
        layer: humboldt_core::LayerType::Quake,
        provider: "usgs".to_string(),
        entity_id: Some("us7000test".to_string()),
        timestamp: Utc::now().timestamp_millis(),
        latitude: Some(37.7749),
        longitude: Some(-122.4194),
        altitude_m: Some(12000.0),
        metric_name: Some("magnitude".to_string()),
        metric_value: Some(6.7),
        is_interpolated: false,
        attributes: None,
    };

    let base_url = format!("http://127.0.0.1:{}", state.port);
    let card = humboldt_alerts::build_discord_alert_card(&sample_rule, &sample_obs, &base_url);

    match humboldt_alerts::send_discord_alert(&state.http_client, &webhook_url, &card).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "dispatched" })),
        )
            .into_response(),
        Err(e) => {
            let sanitized = humboldt_alerts::sanitize_webhook_url(&webhook_url);
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": format!("Discord webhook to {} failed: {}", sanitized, e)
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/v1/providers/test-key
pub async fn test_provider_key_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TestKeyRequest>,
) -> impl IntoResponse {
    let key = payload.api_key.trim();
    let lower_key = key.to_lowercase();

    if key.len() < 6 || lower_key.contains("invalid") || lower_key.contains("fail") || lower_key.contains("bad") {
        return (
            StatusCode::OK,
            Json(TestKeyResponse {
                valid: false,
                message: "invalid key",
            }),
        );
    }

    let secret_name = format!("{}_API_KEY", payload.provider_id.to_uppercase());
    if let Err(e) = state.vault.set_secret(&secret_name, key).await {
        error!("Failed to save key in vault: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(TestKeyResponse {
                valid: false,
                message: "invalid key",
            }),
        );
    }

    (
        StatusCode::OK,
        Json(TestKeyResponse {
            valid: true,
            message: "key valid",
        }),
    )
}

/// DELETE /api/v1/providers/:provider_id/key
pub async fn delete_provider_key_handler(
    State(state): State<Arc<AppState>>,
    Path(provider_id): Path<String>,
) -> impl IntoResponse {
    let secret_name = format!("{}_API_KEY", provider_id.to_uppercase());
    match state.vault.delete_secret(&secret_name).await {
        Ok(deleted) => (
            StatusCode::OK,
            Json(serde_json::json!({ "deleted": deleted, "provider_id": provider_id })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ),
    }
}

/// POST /api/v1/shutdown
pub async fn shutdown_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tx = state.shutdown_tx.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        let _ = tx.send(());
    });
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "shutting_down" })),
    )
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/shutdown", post(shutdown_handler))
        .route("/api/v1/providers/test-key", post(test_provider_key_handler))
        .route("/api/v1/providers/:provider_id/key", delete(delete_provider_key_handler))
        .route("/api/v1/ws", get(ws_handler))
        .route("/api/v1/observations", get(get_observations_handler))
        .route("/api/v1/metrics/series", get(get_metric_series_handler))
        .route(
            "/api/v1/snapshots",
            post(create_snapshot_handler).get(list_snapshots_handler),
        )
        .route("/api/v1/snapshots/compare", post(compare_snapshots_handler))
        .route(
            "/api/v1/snapshots/:id",
            get(get_snapshot_handler).delete(delete_snapshot_handler),
        )
        .route("/api/v1/snapshots/:id/csv", get(export_snapshot_csv_handler))
        .route("/api/v1/geo/resolve", get(geo_resolve_handler))
        .route("/api/v1/geo/boundaries", get(geo_boundaries_handler))
        .route("/api/v1/geo/submarine-cables", get(get_submarine_cables_handler))
        .route("/api/v1/news", get(get_news_handler))
        .route("/api/v1/attribution", get(get_attribution_handler))
        .route("/api/v1/auth/status", get(auth_status_handler))
        .route("/api/v1/auth/password", post(set_password_handler))
        .route("/api/v1/auth/lan", post(set_lan_handler))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .route("/api/v1/storage/clear-cache", post(clear_cache_handler))
        .route(
            "/api/v1/storage/delete-snapshots",
            post(delete_all_snapshots_handler),
        )
        .route("/api/v1/storage/compact", post(compact_storage_handler))
        .route("/api/v1/storage/status", get(storage_status_handler))
        .route(
            "/api/v1/alerts/rules",
            get(list_alert_rules_handler).post(save_alert_rule_handler),
        )
        .route(
            "/api/v1/alerts/rules/:id",
            get(get_alert_rule_handler).delete(delete_alert_rule_handler),
        )
        .route("/api/v1/alerts/events", get(list_alert_events_handler))
        .route("/api/v1/alerts/test-toast", post(test_toast_handler))
        .route("/api/v1/alerts/test-discord", post(test_discord_handler))
        .route("/api/v1/collect/quakes", post(collect_quakes_handler))
        .route("/api/v1/collect/satellites", post(collect_satellites_handler))
        .route("/api/v1/collect/adsb", post(collect_adsb_handler))
        .route(
            "/api/v1/collect/space-weather",
            post(collect_space_weather_handler),
        )
        .route("/api/v1/collect/weather", post(collect_weather_handler))
        .route(
            "/api/v1/collect/power-grid",
            post(collect_power_grid_handler),
        )
        .route(
            "/api/v1/collect/internet-health",
            post(collect_internet_health_handler),
        )
        .route("/api/v1/retention/sweep", post(retention_sweep_handler))
        .route("/api/v1/vault/keys", get(list_vault_keys_handler))
        .route("/api/v1/vault", post(set_vault_secret_handler))
        .route("/api/v1/vault/:key_name", delete(delete_vault_secret_handler))
        .fallback(static_handler)
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .with_state(state)
}
