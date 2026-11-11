use crate::desktop::send_desktop_notification;
use crate::discord::{build_discord_alert_card, send_discord_alert};
use humboldt_core::{AlertRule, ComparisonOp, LayerType, Observation};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AlertEngine {
    rules: Arc<RwLock<HashMap<String, AlertRule>>>,
    http_client: reqwest::Client,
    discord_webhook_url: Arc<RwLock<Option<String>>>,
    base_url: Arc<RwLock<String>>,
    recent_events: Arc<RwLock<Vec<AlertEvent>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AlertEvent {
    pub rule_id: String,
    pub rule_name: String,
    pub timestamp: i64,
    pub layer: LayerType,
    pub metric_name: String,
    pub metric_value: f64,
    pub threshold: f64,
    pub location: Option<String>,
}

impl AlertEngine {
    pub fn new(http_client: reqwest::Client) -> Self {
        let engine = Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            http_client,
            discord_webhook_url: Arc::new(RwLock::new(None)),
            base_url: Arc::new(RwLock::new("http://127.0.0.1:8374".to_string())),
            recent_events: Arc::new(RwLock::new(Vec::new())),
        };

        // Seed default operational alert rules per Specification Section 10
        engine.seed_default_rules();
        engine
    }

    fn seed_default_rules(&self) {
        let default_rules = vec![
            AlertRule {
                id: "rule_major_quake".to_string(),
                name: "Major Earthquake (M6.0+)".to_string(),
                enabled: true,
                layer: LayerType::Quake,
                metric_name: "magnitude".to_string(),
                operator: ComparisonOp::GreaterThan,
                threshold: 6.0,
                region_boundary_id: None,
                cooldown_seconds: 3600,
                last_triggered: None,
                notify_desktop: true,
                notify_discord: true,
            },
            AlertRule {
                id: "rule_space_weather_storm".to_string(),
                name: "Severe Geomagnetic Storm (Kp 6+)".to_string(),
                enabled: true,
                layer: LayerType::SpaceWeather,
                metric_name: "kp_index".to_string(),
                operator: ComparisonOp::GreaterThan,
                threshold: 6.0,
                region_boundary_id: None,
                cooldown_seconds: 3600,
                last_triggered: None,
                notify_desktop: true,
                notify_discord: true,
            },
        ];

        let rules_arc = self.rules.clone();
        tokio::spawn(async move {
            let mut w = rules_arc.write().await;
            for r in default_rules {
                w.insert(r.id.clone(), r);
            }
        });
    }

    pub async fn set_discord_webhook(&self, url: Option<String>) {
        let mut lock = self.discord_webhook_url.write().await;
        *lock = url;
    }

    pub async fn set_base_url(&self, url: String) {
        let mut lock = self.base_url.write().await;
        *lock = url;
    }

    pub async fn list_rules(&self) -> Vec<AlertRule> {
        let lock = self.rules.read().await;
        let mut list: Vec<AlertRule> = lock.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        list
    }

    pub async fn get_rule(&self, id: &str) -> Option<AlertRule> {
        let lock = self.rules.read().await;
        lock.get(id).cloned()
    }

    pub async fn save_rule(&self, rule: AlertRule) -> String {
        let id = if rule.id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            rule.id.clone()
        };
        let mut to_store = rule;
        to_store.id = id.clone();
        let mut lock = self.rules.write().await;
        lock.insert(id.clone(), to_store);
        id
    }

    pub async fn delete_rule(&self, id: &str) -> bool {
        let mut lock = self.rules.write().await;
        lock.remove(id).is_some()
    }

    pub async fn list_recent_events(&self) -> Vec<AlertEvent> {
        let lock = self.recent_events.read().await;
        lock.clone()
    }

    /// Evaluates an observation against active alert rules.
    /// Returns any rules that triggered.
    pub async fn evaluate_observation(&self, obs: &Observation) -> Vec<AlertRule> {
        let val = match obs.metric_value {
            Some(v) => v,
            None => return Vec::new(),
        };

        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut triggered = Vec::new();

        {
            let mut rules_guard = self.rules.write().await;
            for rule in rules_guard.values_mut() {
                if !rule.enabled {
                    continue;
                }

                if rule.layer != obs.layer {
                    continue;
                }

                // If rule specifies metric_name, ensure observation matches
                if let Some(obs_metric) = &obs.metric_name {
                    if !rule.metric_name.is_empty() && &rule.metric_name != obs_metric {
                        continue;
                    }
                }

                if rule.should_trigger(val, now_ms) {
                    rule.last_triggered = Some(now_ms);
                    triggered.push(rule.clone());
                }
            }
        }

        if triggered.is_empty() {
            return Vec::new();
        }

        let webhook_opt = self.discord_webhook_url.read().await.clone();
        let base_url = self.base_url.read().await.clone();

        for rule in &triggered {
            info!(
                "[ALERT TRIGGERED] Rule '{}' (Metric: {} = {:.2}, Threshold: {:.2})",
                rule.name, rule.metric_name, val, rule.threshold
            );

            // 1. Record event
            let event = AlertEvent {
                rule_id: rule.id.clone(),
                rule_name: rule.name.clone(),
                timestamp: now_ms,
                layer: rule.layer,
                metric_name: rule.metric_name.clone(),
                metric_value: val,
                threshold: rule.threshold,
                location: obs.latitude.and_then(|lat| {
                    obs.longitude.map(|lon| format!("{:.4}°, {:.4}°", lat, lon))
                }),
            };
            {
                let mut events = self.recent_events.write().await;
                events.push(event);
                if events.len() > 100 {
                    events.remove(0);
                }
            }

            // 2. Windows Desktop Toast Notification
            if rule.notify_desktop {
                let title = format!("[HUMBOLDT] {}", rule.name);
                let msg = format!(
                    "{} value {:.2} exceeded threshold {:.2}",
                    rule.metric_name, val, rule.threshold
                );
                if let Err(e) = send_desktop_notification(&title, &msg) {
                    warn!("Failed to dispatch desktop toast: {}", e);
                }
            }

            // 3. Discord Webhook Dispatch
            if rule.notify_discord {
                if let Some(ref webhook_url) = webhook_opt {
                    let payload = build_discord_alert_card(rule, obs, &base_url);
                    let client = self.http_client.clone();
                    let url = webhook_url.clone();
                    tokio::spawn(async move {
                        if let Err(e) = send_discord_alert(&client, &url, &payload).await {
                            warn!("Discord alert dispatch failed: {}", e);
                        }
                    });
                }
            }
        }

        triggered
    }
}
