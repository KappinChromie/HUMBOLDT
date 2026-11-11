pub mod launcher;
pub mod port;
pub mod routes;
pub mod single_instance;
pub mod static_files;
pub mod tray;
pub mod workers;

use chrono::Utc;
use humboldt_collectors::{
    parse_adsb_json, parse_ais_json, parse_celestrak_json, parse_gdacs_disasters_geojson,
    parse_nhc_cyclones_json, parse_noaa_aurora_json, parse_safecast_radiation_json,
    parse_usgs_geojson, parse_usgs_volcano_geojson,
};
use humboldt_core::{AppConfig, AppPaths, HumboldtError, Result, WIN32_MUTEX_NAME};
use humboldt_security::{resolve_master_key, Vault};
use humboldt_storage::Storage;
use routes::{create_router, AppState};
use single_instance::{show_native_error_dialog, SingleInstanceMutex};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Seeds offline baseline telemetry from bundled fixtures if database is empty.
async fn seed_offline_telemetry_if_needed(storage: &Storage) -> Result<()> {
    let existing_quakes = storage.observations.query_latest(Some("quake"), 1).await?;
    let existing_sats = storage.observations.query_latest(Some("satellite"), 1).await?;
    let existing_aircraft = storage.observations.query_latest(Some("aircraft"), 1).await?;
    let existing_ships = storage.observations.query_latest(Some("ship"), 1).await?;

    if existing_quakes.is_empty() {
        const QUAKES_FIXTURE: &str = include_str!("../../../fixtures/usgs_quakes.json");
        match parse_usgs_geojson(QUAKES_FIXTURE) {
            Ok(quakes) => {
                info!("Seeding {} offline baseline earthquakes from fixture.", quakes.len());
                let _ = storage.observations.insert_batch(&quakes).await;
            }
            Err(e) => warn!("Failed to parse offline quake fixture: {}", e),
        }
    }

    if existing_sats.is_empty() {
        const SATS_FIXTURE: &str = include_str!("../../../fixtures/celestrak_satellites.json");
        match parse_celestrak_json(SATS_FIXTURE, Utc::now()) {
            Ok(sats) => {
                info!("Seeding and propagating {} offline baseline satellites.", sats.len());
                let _ = storage.observations.insert_batch(&sats).await;
            }
            Err(e) => warn!("Failed to parse offline satellite fixture: {}", e),
        }
    }

    if existing_aircraft.is_empty() {
        const ADSB_FIXTURE: &str = include_str!("../../../fixtures/adsb_aircraft.json");
        match parse_adsb_json(ADSB_FIXTURE, "adsb_lol") {
            Ok(aircraft) => {
                info!("Seeding {} offline baseline aircraft from fixture.", aircraft.len());
                let _ = storage.observations.insert_batch(&aircraft).await;
            }
            Err(e) => warn!("Failed to parse offline ADS-B fixture: {}", e),
        }
    }

    if existing_ships.is_empty() {
        const AIS_FIXTURE: &str = include_str!("../../../fixtures/ais_ships.json");
        match parse_ais_json(AIS_FIXTURE) {
            Ok(ships) => {
                info!("Seeding {} offline baseline vessels from fixture.", ships.len());
                let _ = storage.observations.insert_batch(&ships).await;
            }
            Err(e) => warn!("Failed to parse offline AIS fixture: {}", e),
        }
    }

    let existing_cyclones = storage.observations.query_latest(Some("cyclone"), 1).await?;
    let existing_volcanoes = storage.observations.query_latest(Some("volcano"), 1).await?;
    let existing_radiation = storage.observations.query_latest(Some("radiation"), 1).await?;
    let existing_disasters = storage.observations.query_latest(Some("disaster"), 1).await?;
    let existing_aurora = storage.observations.query_latest(Some("aurora"), 1).await?;

    if existing_cyclones.is_empty() {
        let sample_cyclones = serde_json::json!({
            "activeStorms": [
                {
                    "id": "al092024",
                    "name": "Helene",
                    "classification": "HU",
                    "intensity": 120.0,
                    "pressure": 938.0,
                    "latitude": 27.5,
                    "longitude": -84.5,
                    "movementDir": 15.0,
                    "movementSpeed": 20.0
                },
                {
                    "id": "wp112024",
                    "name": "Yagi",
                    "classification": "TY",
                    "intensity": 130.0,
                    "pressure": 925.0,
                    "latitude": 19.8,
                    "longitude": 109.5,
                    "movementDir": 280.0,
                    "movementSpeed": 12.0
                },
                {
                    "id": "sh162024",
                    "name": "Freddy",
                    "classification": "TC",
                    "intensity": 90.0,
                    "pressure": 965.0,
                    "latitude": -21.2,
                    "longitude": 40.5,
                    "movementDir": 260.0,
                    "movementSpeed": 8.0
                }
            ]
        }).to_string();
        if let Ok(cyclones) = parse_nhc_cyclones_json(&sample_cyclones) {
            info!("Seeding {} offline baseline cyclones.", cyclones.len());
            let _ = storage.observations.insert_batch(&cyclones).await;
        }
    }

    if existing_volcanoes.is_empty() {
        let sample_volcanoes = serde_json::json!({
            "features": [
                {
                    "properties": {
                        "vnum": "332010",
                        "volcano_name": "Kilauea",
                        "color_code": "ORANGE",
                        "alert_level": "WATCH",
                        "elev_m": 1222.0,
                        "observatory": "HVO"
                    },
                    "geometry": { "coordinates": [-155.287, 19.421] }
                },
                {
                    "properties": {
                        "vnum": "321050",
                        "volcano_name": "Mount St. Helens",
                        "color_code": "YELLOW",
                        "alert_level": "ADVISORY",
                        "elev_m": 2549.0,
                        "observatory": "CVO"
                    },
                    "geometry": { "coordinates": [-122.195, 46.191] }
                },
                {
                    "properties": {
                        "vnum": "332020",
                        "volcano_name": "Mauna Loa",
                        "color_code": "GREEN",
                        "alert_level": "NORMAL",
                        "elev_m": 4169.0,
                        "observatory": "HVO"
                    },
                    "geometry": { "coordinates": [-155.602, 19.475] }
                },
                {
                    "properties": {
                        "vnum": "341010",
                        "volcano_name": "Popocatépetl",
                        "color_code": "YELLOW",
                        "alert_level": "ADVISORY",
                        "elev_m": 5426.0,
                        "observatory": "CENAPRED"
                    },
                    "geometry": { "coordinates": [-98.627, 19.022] }
                },
                {
                    "properties": {
                        "vnum": "211060",
                        "volcano_name": "Mount Etna",
                        "color_code": "ORANGE",
                        "alert_level": "WATCH",
                        "elev_m": 3357.0,
                        "observatory": "INGV"
                    },
                    "geometry": { "coordinates": [14.995, 37.751] }
                },
                {
                    "properties": {
                        "vnum": "282080",
                        "volcano_name": "Sakurajima",
                        "color_code": "RED",
                        "alert_level": "WARNING",
                        "elev_m": 1117.0,
                        "observatory": "JMA"
                    },
                    "geometry": { "coordinates": [130.657, 31.585] }
                }
            ]
        }).to_string();
        if let Ok(volcanoes) = parse_usgs_volcano_geojson(&sample_volcanoes) {
            info!("Seeding {} offline baseline active volcanoes.", volcanoes.len());
            let _ = storage.observations.insert_batch(&volcanoes).await;
        }
    }

    if existing_radiation.is_empty() {
        let sample_radiation = serde_json::json!([
            { "id": 101, "value": 36.7, "unit": "cpm", "latitude": 37.422, "longitude": -122.084 },
            { "id": 102, "value": 41.2, "unit": "cpm", "latitude": 40.712, "longitude": -74.006 },
            { "id": 103, "value": 29.5, "unit": "cpm", "latitude": 51.507, "longitude": -0.127 },
            { "id": 104, "value": 38.0, "unit": "cpm", "latitude": 48.856, "longitude": 2.352 },
            { "id": 105, "value": 45.6, "unit": "cpm", "latitude": 35.676, "longitude": 139.650 },
            { "id": 106, "value": 33.1, "unit": "cpm", "latitude": -33.868, "longitude": 151.209 },
            { "id": 107, "value": 48.9, "unit": "cpm", "latitude": 37.774, "longitude": -122.419 }
        ]).to_string();
        if let Ok(rads) = parse_safecast_radiation_json(&sample_radiation) {
            info!("Seeding {} offline baseline radiation monitors.", rads.len());
            let _ = storage.observations.insert_batch(&rads).await;
        }
    }

    if existing_disasters.is_empty() {
        let sample_disasters = serde_json::json!({
            "features": [
                {
                    "properties": {
                        "eventid": "TC100024",
                        "eventtype": "TC",
                        "name": "Tropical Cyclone Helene Emergency",
                        "alertlevel": "Red",
                        "alertscore": 3.0,
                        "country": "United States",
                        "description": "Category 4 Major Hurricane making landfall with catastrophic storm surge."
                    },
                    "geometry": { "coordinates": [-84.5, 29.5] }
                },
                {
                    "properties": {
                        "eventid": "FL200018",
                        "eventtype": "FL",
                        "name": "Danube River Basin Floods",
                        "alertlevel": "Orange",
                        "alertscore": 2.2,
                        "country": "Central Europe",
                        "description": "Severe fluvial flooding across Austria, Czechia, and Poland."
                    },
                    "geometry": { "coordinates": [16.37, 48.21] }
                },
                {
                    "properties": {
                        "eventid": "EQ300055",
                        "eventtype": "EQ",
                        "name": "Noto Peninsula Seismic Sequence",
                        "alertlevel": "Red",
                        "alertscore": 2.8,
                        "country": "Japan",
                        "description": "Shallow M7.5 crustal earthquake with extensive localized tsunami."
                    },
                    "geometry": { "coordinates": [137.2, 37.5] }
                }
            ]
        }).to_string();
        if let Ok(disasters) = parse_gdacs_disasters_geojson(&sample_disasters) {
            info!("Seeding {} offline baseline disaster events.", disasters.len());
            let _ = storage.observations.insert_batch(&disasters).await;
        }
    }

    if existing_aurora.is_empty() {
        if let Ok(aurora) = parse_noaa_aurora_json("{}") {
            info!("Seeding {} baseline auroral oval observation points.", aurora.len());
            let _ = storage.observations.insert_batch(&aurora).await;
        }
    }

    Ok(())
}

/// Core application lifecycle boot sequence.
pub async fn run_app() -> Result<()> {
    // 1. Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting HUMBOLDT v{}...", version());

    // 2. Resolve application paths and ensure directories
    let paths = AppPaths::resolve()?;
    paths.ensure_dirs()?;

    // 3. Win32 Single-Instance Mutex check
    let _instance_guard = match SingleInstanceMutex::try_acquire(WIN32_MUTEX_NAME) {
        Ok(Some(guard)) => guard,
        Ok(None) => {
            // Already running: read active port, open browser, exit 0
            if let Some(active_port) = paths.read_active_port() {
                info!(
                    "Existing HUMBOLDT instance detected on port {}. Launching browser.",
                    active_port
                );
                let _ = open::that(format!("http://127.0.0.1:{}", active_port));
            } else {
                info!("Existing HUMBOLDT instance detected. Launching browser on default port 8374.");
                let _ = open::that("http://127.0.0.1:8374");
            }
            return Ok(());
        }
        Err(err_code) => {
            error!("Failed to create single-instance mutex: error code {}", err_code);
            return Err(HumboldtError::Config(format!(
                "Mutex acquisition failed with code {}",
                err_code
            )));
        }
    };

    // 4. Dynamic Port Allocation (8374 - 8378)
    let config = AppConfig::default();
    let (listener, bound_port) = match port::bind_dynamic_listener(
        config.resolved_bind_address(),
        config.port_range_start,
        config.port_range_end,
    )
    .await
    {
        Ok(pair) => pair,
        Err(err) => {
            let msg = format!(
                "Unable to bind to any port in candidate range {}-{}.\nAnother service may be occupying these ports.",
                config.port_range_start, config.port_range_end
            );
            error!("{}", msg);
            show_native_error_dialog("HUMBOLDT - Port Allocation Error", &msg);
            return Err(err);
        }
    };

    // 5. Record active port to .active_port file
    paths.write_active_port(bound_port)?;
    info!("Active port {} written to {:?}", bound_port, paths.active_port_file);

    // 6. Initialize SQLite Storage Engine and Encrypted Vault
    let storage = Storage::open(&paths.db_path).await?;
    let master_key = resolve_master_key(&paths.data_dir)?;
    let vault = Vault::new(storage.pool.clone(), master_key);

    // Seed offline baseline telemetry if initial database is empty
    if let Err(e) = seed_offline_telemetry_if_needed(&storage).await {
        warn!("Offline telemetry seeding warning: {}", e);
    }

    // 7. Broadcast channel for graceful shutdown coordination
    let (shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    // 8. Initialize system tray (unless headless)
    let _tray_controller = if !launcher::is_headless() {
        Some(tray::SystemTrayController::spawn(
            bound_port,
            shutdown_tx.clone(),
        ))
    } else {
        None
    };

    // 9. Launch default browser
    launcher::launch_browser(bound_port);

    // 10. Live Observation Bus and HTTP Client
    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let (bus, _) = broadcast::channel(4096);
    let geo_index = Arc::new(humboldt_geo::bundled_spatial_index());
    let news_engine = humboldt_news::NewsHierarchyEngine::new(http_client.clone());

    let alert_engine = humboldt_alerts::AlertEngine::new(http_client.clone());
    alert_engine
        .set_base_url(format!("http://127.0.0.1:{}", bound_port))
        .await;
    if let Ok(Some(webhook)) = vault.get_secret("DISCORD_WEBHOOK_URL").await {
        alert_engine
            .set_discord_webhook(Some(webhook.expose_secret().to_string()))
            .await;
    }

    let session_mgr = humboldt_security::SessionManager::new(master_key);
    let lan_password_hash = match vault.get_secret("LAN_PASSWORD_HASH").await {
        Ok(Some(hash)) => Some(hash.expose_secret().to_string()),
        _ => None,
    };

    let app_state = Arc::new(AppState {
        port: bound_port,
        start_time: Instant::now(),
        version: version(),
        storage,
        vault,
        http_client,
        bus,
        geo_index,
        news_engine,
        alert_engine,
        session_mgr,
        lan_enabled: Arc::new(tokio::sync::RwLock::new(false)),
        lan_password_hash: Arc::new(tokio::sync::RwLock::new(lan_password_hash)),
        shutdown_tx: shutdown_tx.clone(),
    });

    // 11. Start background continuous collection and retention workers
    workers::start_background_workers(app_state.clone(), shutdown_tx.clone());

    let router = create_router(app_state);

    info!("HUMBOLDT workstation serving at http://127.0.0.1:{}", bound_port);

    // Axum server with graceful shutdown signal
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received from system tray.");
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received from OS signal.");
            }
        }
    });

    if let Err(e) = server.await {
        error!("Server error: {}", e);
    }

    // Cleanup active port file on exit
    let _ = paths.remove_active_port();
    info!("HUMBOLDT cleanly terminated.");

    Ok(())
}
