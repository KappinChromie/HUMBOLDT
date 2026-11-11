pub mod adsb;
pub mod ais;
pub mod aurora;
pub mod celestrak;
pub mod cyclone;
pub mod disaster;
pub mod internet_health;
pub mod power_grid;
pub mod radiation;
pub mod rate_limiter;
pub mod router;
pub mod space_weather;
pub mod usgs_quakes;
pub mod volcano;
pub mod weather;

pub use adsb::{fetch_adsb, parse_adsb_json, DEFAULT_ADSB_LOL_URL, DEFAULT_AIRPLANES_LIVE_URL};
pub use ais::parse_ais_json;
pub use aurora::{fetch_noaa_aurora, parse_noaa_aurora_json, DEFAULT_NOAA_AURORA_URL};
pub use celestrak::{
    fetch_celestrak_satellites, parse_celestrak_json, propagate_satellite_wgs84,
    DEFAULT_CELESTRAK_URL,
};
pub use cyclone::{fetch_noaa_nhc_cyclones, parse_nhc_cyclones_json, DEFAULT_NOAA_NHC_STORMS_URL};
pub use disaster::{fetch_gdacs_disasters, parse_gdacs_disasters_geojson, DEFAULT_GDACS_DISASTERS_URL};
pub use internet_health::{fetch_caida_ioda, parse_caida_ioda_json, DEFAULT_CAIDA_IODA_URL};
pub use power_grid::{fetch_uk_power_grid, parse_uk_power_grid_json, DEFAULT_UK_CARBON_INTENSITY_URL};
pub use radiation::{fetch_safecast_radiation, parse_safecast_radiation_json, DEFAULT_SAFECAST_API_URL};
pub use rate_limiter::{CircuitBreaker, CircuitState, TokenBucket};
pub use router::{ObservationRouter, ProviderHandle};
pub use space_weather::{fetch_noaa_kp, parse_noaa_kp_json, DEFAULT_NOAA_SWPC_KP_URL};
pub use usgs_quakes::{fetch_usgs_quakes, parse_usgs_geojson, DEFAULT_USGS_URL};
pub use volcano::{fetch_usgs_volcanoes, parse_usgs_volcano_geojson, DEFAULT_USGS_VOLCANO_URL};
pub use weather::{fetch_open_meteo, parse_open_meteo_json};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use humboldt_core::{HumboldtError, LayerType, Observation, RoutingMode};
    use std::sync::Arc;

    fn get_fixture_content(filename: &str) -> String {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join(filename);

        std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|_| panic!("Failed to read fixture: {:?}", fixture_path))
    }

    #[test]
    fn test_parse_usgs_quakes_from_fixture() {
        let json = get_fixture_content("usgs_quakes.json");
        let quakes = parse_usgs_geojson(&json).expect("Parsing USGS fixture failed");
        assert_eq!(quakes.len(), 4);

        let kokopo = quakes
            .iter()
            .find(|q| q.entity_id.as_deref() == Some("us7000m1q1"))
            .expect("Kokopo quake not found");

        assert_eq!(kokopo.layer, LayerType::Quake);
        assert_eq!(kokopo.provider, "usgs_quakes");
        assert_eq!(kokopo.metric_value, Some(6.2));
    }

    #[test]
    fn test_parse_and_propagate_celestrak_from_fixture() {
        let json = get_fixture_content("celestrak_satellites.json");
        let target_time = Utc.with_ymd_and_hms(2024, 11, 9, 20, 0, 0).unwrap();
        let satellites =
            parse_celestrak_json(&json, target_time).expect("Parsing CelesTrak fixture failed");

        assert_eq!(satellites.len(), 3);
        let iss = satellites
            .iter()
            .find(|s| s.entity_id.as_deref() == Some("25544"))
            .expect("ISS not found");

        assert_eq!(iss.layer, LayerType::Satellite);
        assert!((-90.0..=90.0).contains(&iss.latitude.unwrap()));
        assert!((-180.0..=180.0).contains(&iss.longitude.unwrap()));
        assert!(iss.altitude_m.unwrap() > 350_000.0);
    }

    #[test]
    fn test_parse_adsb_from_fixture() {
        let json = get_fixture_content("adsb_aircraft.json");
        let aircraft = parse_adsb_json(&json, "adsb_lol").expect("Parsing ADS-B failed");
        assert_eq!(aircraft.len(), 3);

        let aal100 = aircraft
            .iter()
            .find(|a| a.entity_id.as_deref() == Some("a8234a"))
            .expect("AAL100 not found");

        assert_eq!(aal100.layer, LayerType::Aircraft);
        assert!((aal100.latitude.unwrap() - 39.0997).abs() < 1e-4);
        assert!((aal100.longitude.unwrap() - (-94.5786)).abs() < 1e-4);
        // 35,000 ft * 0.3048 = 10,668 meters
        assert!((aal100.altitude_m.unwrap() - 10668.0).abs() < 1.0);
    }

    #[test]
    fn test_parse_ais_from_fixture() {
        let json = get_fixture_content("ais_ships.json");
        let ships = parse_ais_json(&json).expect("Parsing AIS failed");
        assert_eq!(ships.len(), 2);

        let titan = ships
            .iter()
            .find(|s| s.entity_id.as_deref() == Some("367123456"))
            .expect("PACIFIC TITAN not found");

        assert_eq!(titan.layer, LayerType::Ship);
        assert!((titan.latitude.unwrap() - 29.8123).abs() < 1e-4);
        assert_eq!(titan.metric_value, Some(12.8)); // 12.8 knots SOG
    }

    #[test]
    fn test_parse_noaa_kp_from_fixture() {
        let json = get_fixture_content("noaa_swpc_kp.json");
        let kp_obs = parse_noaa_kp_json(&json).expect("Parsing NOAA SWPC Kp failed");
        assert_eq!(kp_obs.len(), 3);

        let latest = kp_obs.last().unwrap();
        assert_eq!(latest.layer, LayerType::SpaceWeather);
        assert_eq!(latest.metric_name.as_deref(), Some("kp_index"));
        assert_eq!(latest.metric_value, Some(3.33));
    }

    #[test]
    fn test_parse_open_meteo_from_fixture() {
        let json = get_fixture_content("open_meteo_weather.json");
        let weather = parse_open_meteo_json(&json, Some("Kansas City")).expect("Parsing weather failed");
        assert_eq!(weather.len(), 1);

        assert_eq!(weather[0].layer, LayerType::Weather);
        assert_eq!(weather[0].entity_id.as_deref(), Some("Kansas City"));
        assert_eq!(weather[0].metric_value, Some(18.5)); // 18.5 °C
    }

    #[test]
    fn test_parse_uk_power_grid_from_fixture() {
        let json = get_fixture_content("power_grid_uk.json");
        let grid = parse_uk_power_grid_json(&json).expect("Parsing UK Grid failed");
        assert_eq!(grid.len(), 1);

        assert_eq!(grid[0].layer, LayerType::PowerGrid);
        assert_eq!(grid[0].entity_id.as_deref(), Some("UK_NATIONAL_GRID"));
        // Wind 32.7 + solar 0.0 + hydro 1.8 + biomass 5.4 = 39.9%
        assert!((grid[0].metric_value.unwrap() - 39.9).abs() < 1e-2);
    }

    #[test]
    fn test_parse_caida_ioda_from_fixture() {
        let json = get_fixture_content("caida_ioda.json");
        let outages = parse_caida_ioda_json(&json).expect("Parsing CAIDA IODA failed");
        assert_eq!(outages.len(), 2);

        assert_eq!(outages[0].layer, LayerType::InternetHealth);
        assert_eq!(outages[0].entity_id.as_deref(), Some("CU"));
        assert_eq!(outages[0].metric_value, Some(0.82));
    }

    #[tokio::test]
    async fn test_router_failover_mode() {
        // Provider 1 always fails
        let p1 = Arc::new(ProviderHandle {
            slug: "failing_primary".to_string(),
            token_bucket: TokenBucket::new(10.0, 1.0),
            circuit_breaker: CircuitBreaker::new(),
            fetch_fn: Box::new(|| {
                Box::pin(async { Err(HumboldtError::Storage("Simulated HTTP 500".to_string())) })
            }),
        });

        // Provider 2 succeeds
        let p2 = Arc::new(ProviderHandle {
            slug: "healthy_fallback".to_string(),
            token_bucket: TokenBucket::new(10.0, 1.0),
            circuit_breaker: CircuitBreaker::new(),
            fetch_fn: Box::new(|| {
                Box::pin(async {
                    Ok(vec![Observation {
                        id: None,
                        timestamp: 1000,
                        layer: LayerType::Weather,
                        provider: "healthy_fallback".to_string(),
                        entity_id: Some("loc1".to_string()),
                        latitude: Some(0.0),
                        longitude: Some(0.0),
                        altitude_m: None,
                        metric_name: None,
                        metric_value: None,
                        attributes: None,
                        is_interpolated: false,
                    }])
                })
            }),
        });

        let router = ObservationRouter::new(RoutingMode::Failover, vec![p1.clone(), p2]);
        let results = router.execute().await.expect("Failover should succeed on p2");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].provider, "healthy_fallback");
        // Circuit breaker on p1 should be OPEN after failure
        assert!(!p1.circuit_breaker.can_execute());
    }

    #[tokio::test]
    async fn test_router_merge_mode_deduplication() {
        // Provider 1 reports aircraft a1 at t=1000
        let p1 = Arc::new(ProviderHandle {
            slug: "adsb_lol".to_string(),
            token_bucket: TokenBucket::new(10.0, 1.0),
            circuit_breaker: CircuitBreaker::new(),
            fetch_fn: Box::new(|| {
                Box::pin(async {
                    Ok(vec![Observation {
                        id: None,
                        timestamp: 1000,
                        layer: LayerType::Aircraft,
                        provider: "adsb_lol".to_string(),
                        entity_id: Some("a1b2c3".to_string()),
                        latitude: Some(39.0),
                        longitude: Some(-94.0),
                        altitude_m: Some(10000.0),
                        metric_name: None,
                        metric_value: None,
                        attributes: None,
                        is_interpolated: false,
                    }])
                })
            }),
        });

        // Provider 2 reports same aircraft a1 with fresher timestamp t=1005
        let p2 = Arc::new(ProviderHandle {
            slug: "airplanes_live".to_string(),
            token_bucket: TokenBucket::new(10.0, 1.0),
            circuit_breaker: CircuitBreaker::new(),
            fetch_fn: Box::new(|| {
                Box::pin(async {
                    Ok(vec![Observation {
                        id: None,
                        timestamp: 1005,
                        layer: LayerType::Aircraft,
                        provider: "airplanes_live".to_string(),
                        entity_id: Some("a1b2c3".to_string()),
                        latitude: Some(39.01),
                        longitude: Some(-94.01),
                        altitude_m: Some(10020.0),
                        metric_name: None,
                        metric_value: None,
                        attributes: None,
                        is_interpolated: false,
                    }])
                })
            }),
        });

        let router = ObservationRouter::new(RoutingMode::Merge, vec![p1, p2]);
        let results = router.execute().await.expect("Merge should succeed");

        // Duplicate entity_id "a1b2c3" should be unified to 1 record with freshest timestamp
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].entity_id.as_deref(), Some("a1b2c3"));
        assert_eq!(results[0].timestamp, 1005);
        assert_eq!(results[0].provider, "airplanes_live");
    }

    #[test]
    fn test_parse_cyclones() {
        let sample = serde_json::json!({
            "activeStorms": [
                {
                    "id": "al092024",
                    "name": "Helene",
                    "classification": "HU",
                    "latitude": 27.5,
                    "longitude": -84.5,
                    "intensity": 120.0,
                    "pressure": 938.0
                }
            ]
        }).to_string();

        let cyclones = parse_nhc_cyclones_json(&sample).unwrap();
        assert_eq!(cyclones.len(), 1);
        assert_eq!(cyclones[0].layer, LayerType::Cyclone);
        assert_eq!(cyclones[0].provider, "noaa_nhc");
        assert_eq!(cyclones[0].entity_id.as_deref(), Some("al092024"));
        assert_eq!(cyclones[0].metric_value, Some(120.0));
    }

    #[test]
    fn test_parse_volcanoes() {
        let sample = serde_json::json!({
            "features": [
                {
                    "properties": {
                        "vnum": "332010",
                        "volcano_name": "Kilauea",
                        "color_code": "ORANGE",
                        "alert_level": "WATCH",
                        "elev_m": 1222.0
                    },
                    "geometry": {
                        "coordinates": [-155.287, 19.421]
                    }
                }
            ]
        }).to_string();

        let volcanoes = parse_usgs_volcano_geojson(&sample).unwrap();
        assert_eq!(volcanoes.len(), 1);
        assert_eq!(volcanoes[0].layer, LayerType::Volcano);
        assert_eq!(volcanoes[0].provider, "usgs_vhp");
        assert_eq!(volcanoes[0].entity_id.as_deref(), Some("332010"));
        assert_eq!(volcanoes[0].metric_value, Some(2.0)); // WATCH/ORANGE = 2.0
    }

    #[test]
    fn test_parse_radiation() {
        let sample = serde_json::json!([
            {
                "id": 12345,
                "value": 334.0,
                "unit": "cpm",
                "latitude": 37.77,
                "longitude": -122.41
            }
        ]).to_string();

        let rads = parse_safecast_radiation_json(&sample).unwrap();
        assert_eq!(rads.len(), 1);
        assert_eq!(rads[0].layer, LayerType::Radiation);
        assert_eq!(rads[0].provider, "safecast");
        assert_eq!(rads[0].metric_value, Some(1.0)); // 334 cpm / 334 = 1.0 uSv/h
    }

    #[test]
    fn test_parse_disasters() {
        let sample = serde_json::json!({
            "features": [
                {
                    "properties": {
                        "eventid": "TC10001",
                        "eventtype": "TC",
                        "name": "Tropical Cyclone Yagi",
                        "alertlevel": "Red",
                        "alertscore": 3.0,
                        "country": "Vietnam"
                    },
                    "geometry": {
                        "coordinates": [105.8, 21.0]
                    }
                }
            ]
        }).to_string();

        let disasters = parse_gdacs_disasters_geojson(&sample).unwrap();
        assert_eq!(disasters.len(), 1);
        assert_eq!(disasters[0].layer, LayerType::Disaster);
        assert_eq!(disasters[0].provider, "gdacs");
        assert_eq!(disasters[0].metric_value, Some(3.0));
    }

    #[test]
    fn test_parse_aurora() {
        let sample = serde_json::json!({
            "coordinates": [
                [-140.0, 68.0, 75.0],
                [-140.0, 60.0, 10.0] // filtered out (< 25%)
            ]
        }).to_string();

        let aurora = parse_noaa_aurora_json(&sample).unwrap();
        assert_eq!(aurora.len(), 1);
        assert_eq!(aurora[0].layer, LayerType::Aurora);
        assert_eq!(aurora[0].provider, "noaa_swpc");
        assert_eq!(aurora[0].metric_value, Some(75.0));
    }
}
