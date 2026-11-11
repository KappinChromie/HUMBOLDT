pub mod desktop;
pub mod discord;
pub mod engine;

pub use desktop::send_desktop_notification;
pub use discord::{
    build_discord_alert_card, determine_severity_color, sanitize_webhook_url, send_discord_alert,
    DiscordEmbed, DiscordWebhookPayload,
};
pub use engine::{AlertEngine, AlertEvent};

#[cfg(test)]
mod tests {
    use super::*;
    use humboldt_core::{AlertRule, ComparisonOp, LayerType, Observation};

    #[test]
    fn test_webhook_url_sanitization() {
        let valid_webhook = "https://discord.com/api/webhooks/10987654321/secret_token_abc123xyz";
        let sanitized = sanitize_webhook_url(valid_webhook);
        assert_eq!(
            sanitized,
            "https://discord.com/api/webhooks/10987654321/[REDACTED]"
        );
        assert!(!sanitized.contains("secret_token_abc123xyz"));

        let invalid_url = "https://example.com/not/a/webhook";
        assert_eq!(sanitize_webhook_url(invalid_url), "[REDACTED_DISCORD_WEBHOOK]");
    }

    #[test]
    fn test_severity_color_determination() {
        // Red for major reading
        assert_eq!(determine_severity_color(7.5, 5.0), 15158332);
        // Orange for moderate excess
        assert_eq!(determine_severity_color(6.0, 5.0), 15105570);
        // Yellow for close / advisory
        assert_eq!(determine_severity_color(4.2, 4.0), 15844367);
    }

    #[tokio::test]
    async fn test_alert_engine_evaluation_and_cooldown() {
        let client = reqwest::Client::new();
        let engine = AlertEngine::new(client);

        // Add a test rule with 60s cooldown
        let rule = AlertRule {
            id: "test_quake_rule".to_string(),
            name: "Test M5.0+ Quake".to_string(),
            enabled: true,
            layer: LayerType::Quake,
            metric_name: "magnitude".to_string(),
            operator: ComparisonOp::GreaterThan,
            threshold: 5.0,
            region_boundary_id: None,
            cooldown_seconds: 60,
            last_triggered: None,
            notify_desktop: false,
            notify_discord: false,
        };
        engine.save_rule(rule).await;

        let now = chrono::Utc::now().timestamp_millis();
        let obs = Observation {
            id: None,
            layer: LayerType::Quake,
            provider: "usgs".to_string(),
            entity_id: Some("us7000test".to_string()),
            timestamp: now,
            latitude: Some(35.5),
            longitude: Some(-120.5),
            altitude_m: Some(10000.0),
            metric_name: Some("magnitude".to_string()),
            metric_value: Some(5.8),
            is_interpolated: false,
            attributes: None,
        };

        // 1. First observation triggers
        let triggered = engine.evaluate_observation(&obs).await;
        assert_eq!(triggered.len(), 1);
        assert_eq!(triggered[0].id, "test_quake_rule");

        // 2. Immediate second observation is suppressed by 60s cooldown
        let triggered2 = engine.evaluate_observation(&obs).await;
        assert_eq!(triggered2.len(), 0);

        // 3. Below-threshold observation does not trigger
        let mut obs_below = obs.clone();
        obs_below.metric_value = Some(4.2);
        let triggered3 = engine.evaluate_observation(&obs_below).await;
        assert_eq!(triggered3.len(), 0);
    }

    #[test]
    fn test_desktop_notification_no_panic() {
        // Must complete without panicking on Windows or headless
        let res = send_desktop_notification("Test Alert", "Unit test message");
        assert!(res.is_ok());
    }
}
