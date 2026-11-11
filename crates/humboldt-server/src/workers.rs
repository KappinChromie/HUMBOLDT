use chrono::Utc;
use humboldt_collectors::{
    fetch_adsb, fetch_caida_ioda, fetch_noaa_kp, fetch_open_meteo, fetch_uk_power_grid,
    fetch_usgs_quakes, parse_celestrak_json, CircuitBreaker, ObservationRouter, ProviderHandle,
    TokenBucket, DEFAULT_ADSB_LOL_URL, DEFAULT_AIRPLANES_LIVE_URL,
};
use humboldt_core::RoutingMode;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{info, warn};

use crate::routes::AppState;

/// Starts all background continuous ingestion loops and retention tasks.
pub fn start_background_workers(state: Arc<AppState>, shutdown_tx: broadcast::Sender<()>) {
    info!("Starting continuous background ingestion workers...");

    spawn_retention_worker(state.clone(), shutdown_tx.subscribe());
    spawn_quakes_worker(state.clone(), shutdown_tx.subscribe());
    spawn_satellites_worker(state.clone(), shutdown_tx.subscribe());
    spawn_adsb_worker(state.clone(), shutdown_tx.subscribe());
    spawn_ais_worker(state.clone(), shutdown_tx.subscribe());
    spawn_space_weather_worker(state.clone(), shutdown_tx.subscribe());
    spawn_weather_worker(state.clone(), shutdown_tx.subscribe());
    spawn_power_grid_worker(state.clone(), shutdown_tx.subscribe());
    spawn_internet_health_worker(state.clone(), shutdown_tx.subscribe());
    spawn_alerts_worker(state.clone(), shutdown_tx.subscribe());
}

/// Dead reckoning calculation along great circle given speed (knots), heading (degrees), and elapsed time (seconds).
fn dead_reckon_coordinate(lat: f64, lon: f64, speed_knots: f64, heading_deg: f64, elapsed_secs: f64) -> (f64, f64) {
    const EARTH_RADIUS_M: f64 = 6_371_000.0;
    let speed_mps = speed_knots * 0.514444;
    let distance_m = speed_mps * elapsed_secs;
    let delta = distance_m / EARTH_RADIUS_M;
    let theta = heading_deg.to_radians();
    let phi1 = lat.to_radians();
    let lambda1 = lon.to_radians();

    let phi2 = (phi1.sin() * delta.cos() + phi1.cos() * delta.sin() * theta.cos()).asin();
    let lambda2 = lambda1 + (theta.sin() * delta.sin() * phi1.cos()).atan2(delta.cos() - phi1.sin() * phi2.sin());

    let mut new_lon = lambda2.to_degrees();
    while new_lon > 180.0 { new_lon -= 360.0; }
    while new_lon < -180.0 { new_lon += 360.0; }

    (phi2.to_degrees(), new_lon)
}

/// Retention & Disk Cap Sweeper Worker: runs every 10 minutes.
fn spawn_retention_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(600));
        // Skip first immediate tick to allow server startup
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    info!("Running scheduled retention & downsampling sweep...");
                    let now_ms = Utc::now().timestamp_millis();
                    match state.storage.retention.run_sweep(now_ms).await {
                        Ok(stats) => {
                            info!(
                                "Retention sweep finished: downsampled {} live points, generated {} hourly rollups, pruned {} live and {} raw metric records. Initial: {} MB, Final: {} MB",
                                stats.live_tracking_downsampled,
                                stats.metric_series_rolled_up,
                                stats.live_tracking_purged,
                                stats.metric_series_raw_purged,
                                stats.initial_size_bytes / (1024 * 1024),
                                stats.final_size_bytes / (1024 * 1024)
                            );
                        }
                        Err(e) => {
                            warn!("Retention sweep failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Retention worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Earthquakes Worker: polls USGS every 60 seconds.
fn spawn_quakes_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let circuit_breaker = CircuitBreaker::new();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !circuit_breaker.can_execute() {
                        warn!("Quakes collector circuit breaker is OPEN, skipping polling.");
                        continue;
                    }

                    match fetch_usgs_quakes(&state.http_client, None).await {
                        Ok(quakes) => {
                            circuit_breaker.record_success();
                            let count = quakes.len();
                            let _ = state.storage.observations.insert_batch(&quakes).await;
                            for q in quakes {
                                let _ = state.bus.send(Arc::new(q));
                            }
                            info!("Background worker synced {} earthquakes.", count);
                        }
                        Err(e) => {
                            circuit_breaker.record_failure();
                            warn!("Background quakes sync failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Quakes worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Satellites Worker: SGP4 orbital propagation every 5 seconds.
fn spawn_satellites_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    tokio::spawn(async move {
        const DEFAULT_TLE: &str = include_str!("../../../fixtures/celestrak_satellites.json");
        let mut ticker = interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let now = Utc::now();
                    match parse_celestrak_json(DEFAULT_TLE, now) {
                        Ok(sats) => {
                            let _ = state.storage.observations.insert_batch(&sats).await;
                            for s in sats {
                                let _ = state.bus.send(Arc::new(s));
                            }
                        }
                        Err(e) => {
                            warn!("SGP4 propagation failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Satellites worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// ADS-B Aircraft Worker: Merge mode router querying adsb.lol + airplanes.live every 5 seconds,
/// with kinematic dead-reckoning fallback if external providers are unavailable.
fn spawn_adsb_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let client_lol = state.http_client.clone();
    let p_lol = Arc::new(ProviderHandle {
        slug: "adsb_lol".to_string(),
        token_bucket: TokenBucket::new(10.0, 1.0),
        circuit_breaker: CircuitBreaker::new(),
        fetch_fn: Box::new(move || {
            let client = client_lol.clone();
            Box::pin(async move {
                fetch_adsb(&client, DEFAULT_ADSB_LOL_URL, "adsb_lol").await
            })
        }),
    });

    let client_live = state.http_client.clone();
    let p_live = Arc::new(ProviderHandle {
        slug: "airplanes_live".to_string(),
        token_bucket: TokenBucket::new(10.0, 1.0),
        circuit_breaker: CircuitBreaker::new(),
        fetch_fn: Box::new(move || {
            let client = client_live.clone();
            Box::pin(async move {
                fetch_adsb(&client, DEFAULT_AIRPLANES_LIVE_URL, "airplanes_live").await
            })
        }),
    });

    let router = ObservationRouter::new(RoutingMode::Merge, vec![p_lol, p_live]);

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    match router.execute().await {
                        Ok(aircraft) if !aircraft.is_empty() => {
                            let count = aircraft.len();
                            let _ = state.storage.observations.insert_batch(&aircraft).await;
                            for ac in aircraft {
                                let _ = state.bus.send(Arc::new(ac));
                            }
                            info!("Background ADS-B worker synced {} aircraft.", count);
                        }
                        _ => {
                            // Offline or network unavailable: dead-reckon active aircraft forward
                            if let Ok(recent) = state.storage.observations.query_latest(Some("aircraft"), 50).await {
                                if !recent.is_empty() {
                                    let mut updated = Vec::new();
                                    for mut ac in recent {
                                        if let (Some(lat), Some(lon)) = (ac.latitude, ac.longitude) {
                                            let gs = ac.attributes.as_ref()
                                                .and_then(|a| a.get("ground_speed_knots").or_else(|| a.get("gs")))
                                                .and_then(|v| v.as_f64())
                                                .unwrap_or(450.0);
                                            let track = ac.attributes.as_ref()
                                                .and_then(|a| a.get("track_deg").or_else(|| a.get("track")))
                                                .and_then(|v| v.as_f64())
                                                .unwrap_or(180.0);
                                            let (new_lat, new_lon) = dead_reckon_coordinate(lat, lon, gs, track, 5.0);
                                            ac.latitude = Some(new_lat);
                                            ac.longitude = Some(new_lon);
                                            ac.timestamp = Utc::now().timestamp_millis();
                                            ac.is_interpolated = true;
                                            let _ = state.bus.send(Arc::new(ac.clone()));
                                            updated.push(ac);
                                        }
                                    }
                                    let _ = state.storage.observations.insert_batch(&updated).await;
                                }
                            }
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("ADS-B worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// AIS Maritime Vessel Worker: streams and dead-reckons active vessels every 5 seconds.
fn spawn_ais_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Ok(recent) = state.storage.observations.query_latest(Some("ship"), 50).await {
                        if !recent.is_empty() {
                            let mut updated = Vec::new();
                            for mut ship in recent {
                                if let (Some(lat), Some(lon)) = (ship.latitude, ship.longitude) {
                                    let sog = ship.attributes.as_ref()
                                        .and_then(|a| a.get("sog").or_else(|| a.get("speed")))
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(12.0);
                                    let cog = ship.attributes.as_ref()
                                        .and_then(|a| a.get("cog").or_else(|| a.get("heading")))
                                        .and_then(|v| v.as_f64())
                                        .unwrap_or(90.0);
                                    let (new_lat, new_lon) = dead_reckon_coordinate(lat, lon, sog, cog, 5.0);
                                    ship.latitude = Some(new_lat);
                                    ship.longitude = Some(new_lon);
                                    ship.timestamp = Utc::now().timestamp_millis();
                                    ship.is_interpolated = true;
                                    let _ = state.bus.send(Arc::new(ship.clone()));
                                    updated.push(ship);
                                }
                            }
                            let _ = state.storage.observations.insert_batch(&updated).await;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("AIS worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Space Weather Worker: polls NOAA SWPC planetary Kp index every 60 seconds.
fn spawn_space_weather_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let circuit_breaker = CircuitBreaker::new();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !circuit_breaker.can_execute() {
                        warn!("Space weather circuit breaker is OPEN, skipping poll.");
                        continue;
                    }

                    match fetch_noaa_kp(&state.http_client, None).await {
                        Ok(readings) => {
                            circuit_breaker.record_success();
                            let count = readings.len();
                            let _ = state.storage.observations.insert_batch(&readings).await;
                            for r in readings {
                                let _ = state.bus.send(Arc::new(r));
                            }
                            info!("Background space weather worker synced {} Kp readings.", count);
                        }
                        Err(e) => {
                            circuit_breaker.record_failure();
                            warn!("Background space weather sync failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Space weather worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Weather Worker: polls Open-Meteo for reference station every 15 minutes.
fn spawn_weather_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let circuit_breaker = CircuitBreaker::new();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(900));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !circuit_breaker.can_execute() {
                        warn!("Weather circuit breaker is OPEN, skipping poll.");
                        continue;
                    }

                    // Query Kansas City reference station per Master Architecture Specification
                    match fetch_open_meteo(&state.http_client, 39.0997, -94.5786, Some("Kansas City, MO")).await {
                        Ok(obs) => {
                            circuit_breaker.record_success();
                            let count = obs.len();
                            let _ = state.storage.observations.insert_batch(&obs).await;
                            for o in obs {
                                let _ = state.bus.send(Arc::new(o));
                            }
                            info!("Background weather worker synced {} records.", count);
                        }
                        Err(e) => {
                            circuit_breaker.record_failure();
                            warn!("Background weather sync failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Weather worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Power Grid Worker: polls UK National Grid mix every 30 minutes.
fn spawn_power_grid_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let circuit_breaker = CircuitBreaker::new();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(1800));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !circuit_breaker.can_execute() {
                        warn!("Power grid circuit breaker is OPEN, skipping poll.");
                        continue;
                    }

                    match fetch_uk_power_grid(&state.http_client, None).await {
                        Ok(obs) => {
                            circuit_breaker.record_success();
                            let count = obs.len();
                            let _ = state.storage.observations.insert_batch(&obs).await;
                            for o in obs {
                                let _ = state.bus.send(Arc::new(o));
                            }
                            info!("Background power grid worker synced {} records.", count);
                        }
                        Err(e) => {
                            circuit_breaker.record_failure();
                            warn!("Background power grid sync failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Power grid worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Internet Health Worker: polls CAIDA IODA macro outages every 15 minutes.
fn spawn_internet_health_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let circuit_breaker = CircuitBreaker::new();

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(900));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if !circuit_breaker.can_execute() {
                        warn!("Internet health circuit breaker is OPEN, skipping poll.");
                        continue;
                    }

                    match fetch_caida_ioda(&state.http_client, None).await {
                        Ok(obs) => {
                            circuit_breaker.record_success();
                            let count = obs.len();
                            let _ = state.storage.observations.insert_batch(&obs).await;
                            for o in obs {
                                let _ = state.bus.send(Arc::new(o));
                            }
                            info!("Background internet health worker synced {} records.", count);
                        }
                        Err(e) => {
                            circuit_breaker.record_failure();
                            warn!("Background internet health sync failed: {}", e);
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Internet health worker shutting down.");
                    break;
                }
            }
        }
    });
}

/// Alert Rules Evaluation Worker: listens to real-time observation bus and triggers rules.
fn spawn_alerts_worker(state: Arc<AppState>, mut shutdown_rx: broadcast::Receiver<()>) {
    let mut bus_rx = state.bus.subscribe();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = bus_rx.recv() => {
                    match res {
                        Ok(obs) => {
                            state.alert_engine.evaluate_observation(&obs).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            warn!("Alerts worker lagged behind observation bus by {} frames", skipped);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Alerts worker shutting down.");
                    break;
                }
            }
        }
    });
}
