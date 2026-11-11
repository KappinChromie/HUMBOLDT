pub mod alert;
pub mod config;
pub mod error;
pub mod observation;
pub mod paths;
pub mod snapshot;

pub use alert::{AlertRule, ComparisonOp};
pub use config::{
    AppConfig, CANONICAL_USER_AGENT, DEFAULT_PORT_END, DEFAULT_PORT_START, WIN32_MUTEX_NAME,
};
pub use error::{HumboldtError, Result};
pub use observation::{LayerType, Observation, RoutingMode};
pub use paths::AppPaths;
pub use snapshot::{
    compute_comparison_stats, decode_data_points, encode_data_points, ComparisonMode,
    ComparisonStats, DataPoint, Snapshot,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_type_serialization() {
        let layer = LayerType::SpaceWeather;
        let serialized = serde_json::to_string(&layer).unwrap();
        assert_eq!(serialized, "\"space_weather\"");
        let deserialized: LayerType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized, LayerType::SpaceWeather);

        for l in [
            LayerType::Cyclone,
            LayerType::Volcano,
            LayerType::SubmarineCable,
            LayerType::Radiation,
            LayerType::Aurora,
            LayerType::Disaster,
        ] {
            let s = serde_json::to_string(&l).unwrap();
            let d: LayerType = serde_json::from_str(&s).unwrap();
            assert_eq!(d, l);
            assert_eq!(s.trim_matches('"'), l.as_str());
        }
    }

    #[test]
    fn test_observation_validation() {
        let obs = Observation {
            id: None,
            timestamp: 1731200000000,
            layer: LayerType::Aircraft,
            provider: "adsb_lol".to_string(),
            entity_id: Some("a1b2c3".to_string()),
            latitude: Some(39.0997),
            longitude: Some(-94.5786),
            altitude_m: Some(10500.0),
            metric_name: Some("altitude_m".to_string()),
            metric_value: Some(10500.0),
            attributes: Some(serde_json::json!({ "callsign": "AAL123" })),
            is_interpolated: false,
        };

        assert!(obs.is_valid_geo());

        let invalid_obs = Observation {
            latitude: Some(105.0),
            ..obs.clone()
        };
        assert!(!invalid_obs.is_valid_geo());
    }

    #[test]
    fn test_alert_rule_evaluation() {
        let rule = AlertRule {
            id: "test-rule-1".to_string(),
            name: "High Quake".to_string(),
            enabled: true,
            layer: LayerType::Quake,
            metric_name: "magnitude".to_string(),
            operator: ComparisonOp::GreaterThan,
            threshold: 6.0,
            region_boundary_id: None,
            cooldown_seconds: 60,
            last_triggered: Some(1000),
            notify_desktop: true,
            notify_discord: false,
        };

        // Below threshold
        assert!(!rule.should_trigger(5.8, 1000 + 70 * 1000));
        // Above threshold, but within cooldown (1000 + 30s)
        assert!(!rule.should_trigger(6.5, 1000 + 30 * 1000));
        // Above threshold, after cooldown (1000 + 70s)
        assert!(rule.should_trigger(6.5, 1000 + 70 * 1000));
    }

    #[test]
    fn test_app_paths_resolution() {
        let paths = AppPaths::resolve().expect("Failed to resolve app paths");
        assert!(!paths.base_dir.to_string_lossy().is_empty());
        assert!(paths.active_port_file.to_string_lossy().contains(".active_port"));
    }

    #[test]
    fn test_app_config_defaults() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.port_range_start, 8374);
        assert_eq!(cfg.port_range_end, 8378);
        assert_eq!(cfg.bind_address, "127.0.0.1");
        assert_eq!(cfg.resolved_bind_address(), "127.0.0.1");
    }

    #[test]
    fn test_snapshot_compression_and_csv() {
        let points = vec![
            DataPoint {
                timestamp: 1730419200000,
                value: 15.2,
            },
            DataPoint {
                timestamp: 1730422800000,
                value: 16.8,
            },
        ];

        let snap = Snapshot::new(
            "test-uuid-1".to_string(),
            "KC Temperature".to_string(),
            1730419200000,
            "weather".to_string(),
            "temp_c".to_string(),
            Some("Kansas City, MO".to_string()),
            1730419200000,
            1730422800000,
            "raw".to_string(),
            "{}".to_string(),
            &points,
        )
        .expect("Snapshot creation failed");

        let decoded = snap.decoded_points().expect("Decompression failed");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].value, 15.2);
        assert_eq!(decoded[1].value, 16.8);

        let csv = snap.to_csv().expect("CSV generation failed");
        assert!(csv.contains("timestamp_ms,iso_time,value"));
        assert!(csv.contains("1730419200000"));
        assert!(csv.contains("15.2"));
    }

    #[test]
    fn test_statistical_divergence_engine() {
        // Series A: [10.0, 20.0, 30.0] -> mean=20, min=10, max=30, range=20
        let series_a = vec![
            DataPoint { timestamp: 1000, value: 10.0 },
            DataPoint { timestamp: 2000, value: 20.0 },
            DataPoint { timestamp: 3000, value: 30.0 },
        ];

        // Series B: [12.0, 22.0, 32.0] -> mean=22, min=12, max=32
        // Diffs: [2, 2, 2] -> sum(sq) = 12 -> mean_sq = 4 -> rmse = 2
        // NRMSD = 2.0 / (30.0 - 10.0) = 2.0 / 20.0 = 0.10 (10%)
        let series_b = vec![
            DataPoint { timestamp: 1000, value: 12.0 },
            DataPoint { timestamp: 2000, value: 22.0 },
            DataPoint { timestamp: 3000, value: 32.0 },
        ];

        let stats = compute_comparison_stats(&series_a, &series_b, ComparisonMode::Calendar);
        assert_eq!(stats.sample_count, 3);
        assert!((stats.delta_mean - 2.0).abs() < 1e-6);
        assert!((stats.delta_max - 2.0).abs() < 1e-6);
        assert!((stats.delta_min - 2.0).abs() < 1e-6);
        assert!((stats.nrmsd - 0.10).abs() < 1e-6);
    }
}

