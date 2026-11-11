use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use humboldt_collectors::{parse_celestrak_json, parse_usgs_geojson};
use humboldt_security::Vault;
use humboldt_server::port::bind_dynamic_listener;
use humboldt_server::routes::{create_router, AppState};
use humboldt_server::single_instance::SingleInstanceMutex;
use humboldt_server::static_files::EmbeddedAssets;
use humboldt_storage::Storage;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceExt;

async fn setup_test_app_state() -> Arc<AppState> {
    let storage = Storage::in_memory().await.unwrap();
    let master_key = [42u8; 32];
    let vault = Vault::new(storage.pool.clone(), master_key);
    let http_client = reqwest::Client::new();
    let (bus, _) = tokio::sync::broadcast::channel(100);
    let geo_index = Arc::new(humboldt_geo::bundled_spatial_index());
    let news_engine = humboldt_news::NewsHierarchyEngine::new(http_client.clone());
    let alert_engine = humboldt_alerts::AlertEngine::new(http_client.clone());
    let session_mgr = humboldt_security::SessionManager::new(master_key);
    let lan_enabled = Arc::new(tokio::sync::RwLock::new(false));
    let lan_password_hash = Arc::new(tokio::sync::RwLock::new(None));

    Arc::new(AppState {
        port: 8374,
        start_time: Instant::now(),
        version: "0.1.0",
        storage,
        vault,
        http_client,
        bus,
        geo_index,
        news_engine,
        alert_engine,
        session_mgr,
        lan_enabled,
        lan_password_hash,
        shutdown_tx: tokio::sync::broadcast::channel(1).0,
    })
}

#[tokio::test]
async fn test_health_endpoint() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["status"], "nominal");
    assert_eq!(json["port"], 8374);
    assert_eq!(json["version"], "0.1.0");
}

#[tokio::test]
async fn test_observations_api_and_persistence() {
    let state = setup_test_app_state().await;

    // Load fixture quakes and satellites into storage
    const QUAKES_JSON: &str = include_str!("../../../fixtures/usgs_quakes.json");
    const SATS_JSON: &str = include_str!("../../../fixtures/celestrak_satellites.json");

    let quakes = parse_usgs_geojson(QUAKES_JSON).unwrap();
    let sats = parse_celestrak_json(SATS_JSON, Utc::now()).unwrap();

    state.storage.observations.insert_batch(&quakes).await.unwrap();
    state.storage.observations.insert_batch(&sats).await.unwrap();

    let router = create_router(state);

    // Query quakes
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/observations?layer=quake&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["layer"], "quake");
    assert!(items[0]["metric_value"].as_f64().unwrap() > 0.0);

    // Query satellites
    let response_sats = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/observations?layer=satellite&limit=10")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_sats.status(), StatusCode::OK);
    let body_sats = axum::body::to_bytes(response_sats.into_body(), usize::MAX)
        .await
        .unwrap();
    let sat_items: Vec<serde_json::Value> = serde_json::from_slice(&body_sats).unwrap();
    assert_eq!(sat_items.len(), 3);
    assert_eq!(sat_items[0]["layer"], "satellite");
    assert!(sat_items[0]["altitude_m"].as_f64().unwrap() > 300_000.0);
}

#[tokio::test]
async fn test_vault_api_lifecycle() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. Post secret
    let post_req = Request::builder()
        .method("POST")
        .uri("/api/v1/vault")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({
                "key_name": "NASA_FIRMS_KEY",
                "secret_value": "secret_token_12345"
            })
            .to_string(),
        ))
        .unwrap();

    let post_res = router.clone().oneshot(post_req).await.unwrap();
    assert_eq!(post_res.status(), StatusCode::CREATED);

    // 2. List keys
    let list_req = Request::builder()
        .uri("/api/v1/vault/keys")
        .body(Body::empty())
        .unwrap();
    let list_res = router.clone().oneshot(list_req).await.unwrap();
    assert_eq!(list_res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let keys: Vec<String> = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(keys, vec!["NASA_FIRMS_KEY"]);

    // 3. Delete secret
    let del_req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/vault/NASA_FIRMS_KEY")
        .body(Body::empty())
        .unwrap();
    let del_res = router.oneshot(del_req).await.unwrap();
    assert_eq!(del_res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_static_asset_serving_index() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));

    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body_bytes);
    assert!(body_str.contains("HUMBOLDT"));
}

#[tokio::test]
async fn test_spa_fallback_routing() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/globe/view/kansas-city")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(content_type.contains("text/html"));
}

#[test]
fn test_tier1_bundled_natural_earth_assets() {
    let tilemap_resource =
        EmbeddedAssets::get("cesium/Assets/Textures/NaturalEarthII/tilemapresource.xml");
    assert!(
        tilemap_resource.is_some(),
        "Tier 1 Natural Earth tilemapresource.xml must be embedded"
    );

    let tile_0_0 = EmbeddedAssets::get("cesium/Assets/Textures/NaturalEarthII/0/0/0.jpg");
    assert!(
        tile_0_0.is_some(),
        "Tier 1 Natural Earth base root tile must be embedded"
    );
}

#[tokio::test]
async fn test_dynamic_port_range_binding() {
    let (listener, port) = bind_dynamic_listener("127.0.0.1", 19374, 19380)
        .await
        .expect("Should bind to port in range");

    assert!((19374..=19380).contains(&port));
    drop(listener);
}

#[test]
fn test_single_instance_mutex_acquisition() {
    let test_mutex_name = "Global\\HumboldtTestMutex_M2";
    let guard1 = SingleInstanceMutex::try_acquire(test_mutex_name);
    assert!(guard1.is_ok(), "First acquisition should succeed");

    #[cfg(windows)]
    {
        if let Ok(Some(_g1)) = guard1 {
            let guard2 = SingleInstanceMutex::try_acquire(test_mutex_name);
            assert!(
                matches!(guard2, Ok(None)),
                "Second acquisition on existing mutex must return None"
            );
        }
    }
}

#[tokio::test]
async fn test_retention_sweep_api() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/retention/sweep")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(json["status"], "completed");
    assert!(json.get("live_tracking_downsampled").is_some());
    assert!(json.get("metric_series_rolled_up").is_some());
}

#[tokio::test]
async fn test_live_observation_bus_broadcast() {
    let state = setup_test_app_state().await;
    let mut rx = state.bus.subscribe();

    let obs = humboldt_core::Observation {
        id: None,
        timestamp: Utc::now().timestamp_millis(),
        layer: humboldt_core::LayerType::Aircraft,
        provider: "adsb_lol".to_string(),
        entity_id: Some("A1B2C3".to_string()),
        latitude: Some(39.1),
        longitude: Some(-94.5),
        altitude_m: Some(10500.0),
        metric_name: Some("ground_speed_knots".to_string()),
        metric_value: Some(450.0),
        is_interpolated: false,
        attributes: None,
    };

    let send_res = state.bus.send(Arc::new(obs.clone()));
    assert!(send_res.is_ok());

    let received = rx.recv().await.expect("Failed to receive from bus");
    assert_eq!(received.entity_id.as_deref(), Some("A1B2C3"));
    assert_eq!(received.layer, humboldt_core::LayerType::Aircraft);
}

#[tokio::test]
async fn test_geo_resolve_api_kansas_city() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let start = std::time::Instant::now();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=39.0997&lon=-94.5786")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 50,
        "Geo resolve API must be sub-5ms on logic (total oneshot under 50ms): {:?}",
        elapsed
    );

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(json["country"], "United States");
    assert_eq!(json["state"], "Missouri");
    assert_eq!(json["county"], "Jackson County");
    assert_eq!(json["city"], "Kansas City");
}

#[tokio::test]
async fn test_geo_resolve_api_open_ocean() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=-40.0&lon=-140.0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(json["country"].is_null());
    assert!(json["city"].is_null());
}

#[tokio::test]
async fn test_geo_resolve_api_zoom_levels() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. Really zoomed out: target_level=country
    let res_country = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=39.0997&lon=-94.5786&target_level=country")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_country.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res_country.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["country"], "United States");
    assert!(json["state"].is_null());
    assert!(json["county"].is_null());
    assert!(json["city"].is_null());

    // 2. Zoomed in some: target_level=state
    let res_state = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=39.0997&lon=-94.5786&target_level=state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_state.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res_state.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["country"], "United States");
    assert_eq!(json["state"], "Missouri");
    assert!(json["county"].is_null());
    assert!(json["city"].is_null());

    // 3. Zoomed in more: target_level=county
    let res_county = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=39.0997&lon=-94.5786&target_level=county")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_county.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res_county.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["country"], "United States");
    assert_eq!(json["state"], "Missouri");
    assert_eq!(json["county"], "Jackson County");
    assert!(json["city"].is_null());

    // 4. Zoomed in close: target_level=city
    let res_city = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/resolve?lat=39.0997&lon=-94.5786&target_level=city")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_city.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(res_city.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["country"], "United States");
    assert_eq!(json["state"], "Missouri");
    assert_eq!(json["county"], "Jackson County");
    assert_eq!(json["city"], "Kansas City");
}

#[tokio::test]
async fn test_snapshot_lifecycle_and_csv_export() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. Create a snapshot with 5 telemetry points
    let payload = serde_json::json!({
        "name": "KC Temperature Oct 2026",
        "layer": "weather",
        "metric_name": "temperature_c",
        "query_entity": "Kansas City",
        "start_time": 1727740800000i64,
        "end_time": 1730419200000i64,
        "resolution": "1h_rollup",
        "metadata_json": "{\"unit\":\"celsius\",\"provider\":\"open-meteo\"}",
        "data_points": [
            { "timestamp": 1727740800000i64, "value": 15.0 },
            { "timestamp": 1727744400000i64, "value": 16.5 },
            { "timestamp": 1727748000000i64, "value": 18.0 },
            { "timestamp": 1727751600000i64, "value": 20.0 },
            { "timestamp": 1727755200000i64, "value": 19.5 }
        ]
    });

    let create_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/snapshots")
                .header("content-type", "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_resp.status(), StatusCode::CREATED);
    let create_bytes = axum::body::to_bytes(create_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let created: serde_json::Value = serde_json::from_slice(&create_bytes).unwrap();
    let snap_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["name"], "KC Temperature Oct 2026");
    assert_eq!(created["point_count"], 5);

    // 2. List snapshots
    let list_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/snapshots")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], snap_id);
    assert_eq!(list[0]["point_count"], 5);

    // 3. Get single snapshot with decoded points
    let get_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/snapshots/{}", snap_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_resp.status(), StatusCode::OK);
    let get_bytes = axum::body::to_bytes(get_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let single: serde_json::Value = serde_json::from_slice(&get_bytes).unwrap();
    let points = single["data_points"].as_array().unwrap();
    assert_eq!(points.len(), 5);
    assert_eq!(points[0]["value"], 15.0);
    assert_eq!(points[4]["value"], 19.5);

    // 4. Download RFC 4180 CSV
    let csv_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/snapshots/{}/csv", snap_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(csv_resp.status(), StatusCode::OK);
    assert_eq!(
        csv_resp.headers().get("content-type").unwrap(),
        "text/csv; charset=utf-8"
    );
    let csv_bytes = axum::body::to_bytes(csv_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let csv_text = String::from_utf8(csv_bytes.to_vec()).unwrap();
    assert!(csv_text.starts_with("timestamp_ms,iso_time,value\n"));
    assert!(csv_text.contains("1727740800000,"));
    assert!(csv_text.contains(",15\n") || csv_text.contains(",15.0\n") || csv_text.contains(",15"));

    // 5. Delete snapshot
    let del_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/snapshots/{}", snap_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(del_resp.status(), StatusCode::OK);

    // 6. Verify 404 after deletion
    let get_again = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/snapshots/{}", snap_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_again.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_snapshot_comparison_engine() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // Create Snapshot A (2025)
    let payload_a = serde_json::json!({
        "name": "Oct 2025 Series",
        "layer": "power_grid",
        "metric_name": "generation_mw",
        "data_points": [
            { "timestamp": 1000, "value": 10.0 },
            { "timestamp": 2000, "value": 20.0 },
            { "timestamp": 3000, "value": 30.0 }
        ]
    });
    let resp_a = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/snapshots")
                .header("content-type", "application/json")
                .body(Body::from(payload_a.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let id_a = axum::body::to_bytes(resp_a.into_body(), usize::MAX)
        .await
        .unwrap();
    let id_a: serde_json::Value = serde_json::from_slice(&id_a).unwrap();
    let snap_a_id = id_a["id"].as_str().unwrap();

    // Create Snapshot B (2026, +5.0 constant offset)
    let payload_b = serde_json::json!({
        "name": "Oct 2026 Series",
        "layer": "power_grid",
        "metric_name": "generation_mw",
        "data_points": [
            { "timestamp": 10000, "value": 15.0 },
            { "timestamp": 11000, "value": 25.0 },
            { "timestamp": 12000, "value": 35.0 }
        ]
    });
    let resp_b = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/snapshots")
                .header("content-type", "application/json")
                .body(Body::from(payload_b.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let id_b = axum::body::to_bytes(resp_b.into_body(), usize::MAX)
        .await
        .unwrap();
    let id_b: serde_json::Value = serde_json::from_slice(&id_b).unwrap();
    let snap_b_id = id_b["id"].as_str().unwrap();

    // Compare with Relative mode
    let compare_req = serde_json::json!({
        "snapshot_a_id": snap_a_id,
        "snapshot_b_id": snap_b_id,
        "mode": "relative"
    });
    let comp_resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/snapshots/compare")
                .header("content-type", "application/json")
                .body(Body::from(compare_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(comp_resp.status(), StatusCode::OK);
    let comp_bytes = axum::body::to_bytes(comp_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let comp: serde_json::Value = serde_json::from_slice(&comp_bytes).unwrap();

    let stats = &comp["stats"];
    // delta_mean = 25.0 - 20.0 = 5.0
    assert!((stats["delta_mean"].as_f64().unwrap() - 5.0).abs() < 1e-6);
    // delta_max = 35.0 - 30.0 = 5.0
    assert!((stats["delta_max"].as_f64().unwrap() - 5.0).abs() < 1e-6);
    // delta_min = 15.0 - 10.0 = 5.0
    assert!((stats["delta_min"].as_f64().unwrap() - 5.0).abs() < 1e-6);
    // NRMSD = 5.0 / (30.0 - 10.0) = 5.0 / 20.0 = 0.25
    assert!((stats["nrmsd"].as_f64().unwrap() - 0.25).abs() < 1e-6);
    assert_eq!(stats["sample_count"], 3);

    // Verify relative series timestamps start at 0
    let s_a = comp["series_a"].as_array().unwrap();
    let s_b = comp["series_b"].as_array().unwrap();
    assert_eq!(s_a[0]["timestamp"], 0);
    assert_eq!(s_b[0]["timestamp"], 0);
    assert_eq!(s_a[1]["timestamp"], 1000);
    assert_eq!(s_b[1]["timestamp"], 1000);
}

#[tokio::test]
async fn test_metric_series_query_endpoint() {
    let state = setup_test_app_state().await;

    // Insert observations
    let now = Utc::now().timestamp_millis();
    let obs = humboldt_core::Observation {
        id: None,
        layer: humboldt_core::LayerType::PowerGrid,
        provider: "uk_elexon".to_string(),
        entity_id: Some("wind".to_string()),
        timestamp: now,
        latitude: None,
        longitude: None,
        altitude_m: None,
        metric_name: Some("wind".to_string()),
        metric_value: Some(8500.0),
        is_interpolated: false,
        attributes: None,
    };
    state.storage.observations.insert_batch(&[obs]).await.unwrap();

    let router = create_router(state);
    let resp = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/metrics/series?layer=power_grid&metric_name=wind&from={}&to={}", now - 1000, now + 1000))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let points: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0]["value"], 8500.0);
}

#[tokio::test]
async fn test_lan_auth_lifecycle_and_rate_limiting() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. Initial status: LAN disabled, no password
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status["lan_enabled"], false);
    assert_eq!(status["password_configured"], false);
    assert_eq!(status["authenticated"], false);

    // 2. Cannot enable LAN mode before configuring password (Section 4.2 gating)
    let enable_req = serde_json::json!({ "enabled": true });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/lan")
                .header("content-type", "application/json")
                .body(Body::from(enable_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // 3. Configure master access password
    let set_pwd_req = serde_json::json!({ "password": "HumboldtStation2026!" });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/password")
                .header("content-type", "application/json")
                .body(Body::from(set_pwd_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Now enable LAN mode
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/lan")
                .header("content-type", "application/json")
                .body(Body::from(enable_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Failed login attempt (wrong password)
    let bad_login = serde_json::json!({ "password": "WrongPassword!" });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "192.168.1.100")
                .body(Body::from(bad_login.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 5b. Immediate retry from 192.168.1.100 must be rate-limited (HTTP 429)
    let resp_rate_limited = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "192.168.1.100")
                .body(Body::from(bad_login.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp_rate_limited.status(), StatusCode::TOO_MANY_REQUESTS);

    // 6. Successful login from clean client IP (192.168.1.101)
    let good_login = serde_json::json!({ "password": "HumboldtStation2026!" });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header("content-type", "application/json")
                .header("x-forwarded-for", "192.168.1.101")
                .body(Body::from(good_login.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let cookie_header = resp
        .headers()
        .get("set-cookie")
        .expect("Must return set-cookie")
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie_header.contains("humboldt_session="));
    assert!(cookie_header.contains("HttpOnly"));
    assert!(cookie_header.contains("SameSite=Strict"));
    // Explicitly verify Secure flag is omitted for plain HTTP LAN mode
    assert!(
        !cookie_header.contains("Secure"),
        "LAN session cookie must omit Secure flag"
    );

    // 7. Verify authenticated status with cookie
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/status")
                .header("cookie", &cookie_header)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status["lan_enabled"], true);
    assert_eq!(status["password_configured"], true);
    assert_eq!(status["authenticated"], true);

    // 8. Logout
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let logout_cookie = resp.headers().get("set-cookie").unwrap().to_str().unwrap();
    assert!(logout_cookie.contains("Max-Age=0"));
}

#[tokio::test]
async fn test_storage_reclamation_endpoints() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. Storage Status
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/storage/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let status: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(status["disk_cap_bytes"].as_u64().unwrap() > 0);

    // 2. Clear Caches
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/storage/clear-cache")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Delete All Snapshots
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/storage/delete-snapshots")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Compact Database
    let resp = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/storage/compact")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_alert_rules_and_toast_subsystem() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    // 1. List default rules
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/alerts/rules")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Create custom rule
    let rule_payload = serde_json::json!({
        "id": "custom_quake_rule",
        "name": "Custom M6.5+ Quake",
        "enabled": true,
        "layer": "quake",
        "metric_name": "magnitude",
        "operator": "greater_than",
        "threshold": 6.5,
        "cooldown_seconds": 60,
        "notify_desktop": true,
        "notify_discord": false
    });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/alerts/rules")
                .header("content-type", "application/json")
                .body(Body::from(rule_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 3. Test Desktop Toast
    let toast_req = serde_json::json!({
        "title": "[HUMBOLDT] Seismic Watch",
        "message": "Magnitude 6.7 registered off coast of Eureka, CA"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/alerts/test-toast")
                .header("content-type", "application/json")
                .body(Body::from(toast_req.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Test Discord Webhook with URL sanitization verification
    let bad_discord = serde_json::json!({
        "webhook_url": "https://discord.com/api/webhooks/9876543210/very_secret_webhook_token_123"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/alerts/test-discord")
                .header("content-type", "application/json")
                .body(Body::from(bad_discord.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Must be either BAD_GATEWAY or OK (if network worked), but must NEVER leak the secret token in body
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(
        !body_str.contains("very_secret_webhook_token_123"),
        "Discord webhook token must be strictly sanitized from error responses"
    );
}

#[tokio::test]
async fn test_provider_attribution_endpoint() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/attribution")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Vec<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();

    // Verify key providers and licenses are acknowledged
    assert!(list.len() >= 15);
    assert!(list.iter().any(|p| p["name"] == "USGS Earthquakes" && p["license_type"] == "Public Domain"));
    assert!(list.iter().any(|p| p["name"] == "Open-Meteo" && p["license_type"] == "CC BY 4.0"));
    assert!(list.iter().any(|p| p["name"] == "CelesTrak" && p["url"].as_str().unwrap().contains("celestrak")));
    assert!(list.iter().any(|p| p["name"] == "UK National Grid ESO" && p["license_type"] == "CC BY 4.0"));
    assert!(list.iter().any(|p| p["name"] == "adsb.lol" && p["url"].as_str().unwrap().contains("adsb.lol")));
    // Verify each provider has an id
    assert!(list.iter().all(|p| p.get("id").and_then(|v| v.as_str()).is_some()));
}

#[tokio::test]
async fn test_provider_key_testing_and_vault_lifecycle() {
    let state = setup_test_app_state().await;
    let router = create_router(state.clone());

    // 1. Test invalid key (too short or bad keyword)
    let invalid_payload = serde_json::json!({
        "provider_id": "open_meteo",
        "api_key": "bad_key"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/providers/test-key")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(invalid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["valid"], false);
    assert_eq!(body["message"], "invalid key");

    // 2. Test valid key
    let valid_payload = serde_json::json!({
        "provider_id": "open_meteo",
        "api_key": "commercial_om_key_987654321"
    });
    let resp = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/providers/test-key")
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(valid_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["valid"], true);
    assert_eq!(body["message"], "key valid");

    // Verify key was saved in vault
    let stored = state.vault.get_secret("OPEN_METEO_API_KEY").await.unwrap();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().expose_secret(), "commercial_om_key_987654321");

    // 3. Delete provider key via Change Key API
    let resp = router
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/providers/open_meteo/key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let stored_after = state.vault.get_secret("OPEN_METEO_API_KEY").await.unwrap();
    assert!(stored_after.is_none());
}

#[tokio::test]
async fn test_shutdown_endpoint() {
    let state = setup_test_app_state().await;
    let mut shutdown_rx = state.shutdown_tx.subscribe();
    let router = create_router(state);

    let resp = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/shutdown")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "shutting_down");

    // Verify shutdown signal is received
    tokio::time::timeout(std::time::Duration::from_millis(500), shutdown_rx.recv())
        .await
        .expect("Shutdown signal should arrive within 500ms")
        .expect("Shutdown channel should receive message");
}

#[tokio::test]
async fn test_geo_boundaries_api() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/boundaries")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let countries = body["countries"].as_array().expect("countries array");
    let states = body["states"].as_array().expect("states array");
    let counties = body["counties"].as_array().expect("counties array");
    let places = body["places"].as_array().expect("places array");

    assert!(countries.len() >= 10, "Should have bundled countries");
    assert!(states.len() >= 10, "Should have bundled states");
    assert!(counties.len() >= 10, "Should have bundled counties");
    assert!(
        body["cities"].as_array().map_or(0, |c| c.len()) >= 15,
        "Should have bundled city municipal boundaries"
    );
    assert!(places.len() >= 20, "Should have bundled places");

    // Check United States polygon
    let us = countries.iter().find(|c| c["name"] == "United States").expect("US polygon");
    assert_eq!(us["level"], 0);
    assert_eq!(us["code"], "US");
    assert!(!us["rings"].as_array().unwrap().is_empty());

    // Check Kansas City place
    let kc = places.iter().find(|p| p["name"] == "Kansas City").expect("KC place");
    assert_eq!(kc["country_code"], "US");
    assert_eq!(kc["state_code"], "MO");
    assert!((kc["lat"].as_f64().unwrap() - 39.0997).abs() < 0.01);
}

#[tokio::test]
async fn test_submarine_cables_api() {
    let state = setup_test_app_state().await;
    let router = create_router(state);

    let resp = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/geo/submarine-cables")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let cables: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let arr = cables.as_array().expect("cables array");

    assert!(arr.len() >= 10, "Should return at least 10 major global cables");
    let marea = arr.iter().find(|c| c["id"] == "marea").expect("MAREA cable");
    assert_eq!(marea["name"], "MAREA");
    assert!(marea["coordinates"].as_array().unwrap().len() >= 5);
}





