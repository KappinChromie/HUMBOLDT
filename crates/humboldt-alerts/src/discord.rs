use humboldt_core::{AlertRule, HumboldtError, Observation, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmbedField {
    pub name: String,
    pub value: String,
    pub inline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmbed {
    pub title: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub color: u32,
    pub fields: Vec<DiscordEmbedField>,
    pub timestamp: Option<String>,
    pub footer: Option<DiscordEmbedFooter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordEmbedFooter {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordWebhookPayload {
    pub username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub embeds: Vec<DiscordEmbed>,
}

/// Sanitizes a Discord webhook URL by replacing the sensitive webhook token with [REDACTED].
/// Strictly ensures sensitive tokens are never exposed in error logs or debug dumps.
pub fn sanitize_webhook_url(url: &str) -> String {
    if let Some(idx) = url.find("/api/webhooks/") {
        let prefix = &url[..idx + "/api/webhooks/".len()];
        let rest = &url[idx + "/api/webhooks/".len()..];
        if let Some(slash_idx) = rest.find('/') {
            let id = &rest[..slash_idx];
            return format!("{}{}/[REDACTED]", prefix, id);
        }
    }
    "[REDACTED_DISCORD_WEBHOOK]".to_string()
}

/// Calculates embed color based on severity:
/// - Yellow (0xF1C40F = 15844367): Advisory
/// - Orange (0xE67E22 = 15105570): Watch
/// - Red    (0xE74C3C = 15158332): Warning
pub fn determine_severity_color(reading: f64, threshold: f64) -> u32 {
    let diff = reading - threshold;
    if diff >= 2.0 || reading >= 7.0 {
        15158332 // Red (Warning)
    } else if diff >= 1.0 || reading >= 5.0 {
        15105570 // Orange (Watch)
    } else {
        15844367 // Yellow (Advisory)
    }
}

/// Builds a rich Discord webhook card from an alert rule and the triggering observation.
pub fn build_discord_alert_card(
    rule: &AlertRule,
    obs: &Observation,
    base_url: &str,
) -> DiscordWebhookPayload {
    let reading = obs.metric_value.unwrap_or(rule.threshold);
    let color = determine_severity_color(reading, rule.threshold);

    let location_str = match (obs.latitude, obs.longitude) {
        (Some(lat), Some(lon)) => format!("{:.4}°, {:.4}°", lat, lon),
        _ => obs.entity_id.clone().unwrap_or_else(|| "Global / Regional".to_string()),
    };

    let deep_link = match (obs.latitude, obs.longitude) {
        (Some(lat), Some(lon)) => format!("{}/?lat={}&lon={}", base_url, lat, lon),
        _ => base_url.to_string(),
    };

    let op_symbol = match rule.operator {
        humboldt_core::ComparisonOp::GreaterThan => ">",
        humboldt_core::ComparisonOp::LessThan => "<",
        humboldt_core::ComparisonOp::Equal => "=",
    };

    let iso_timestamp = chrono::DateTime::from_timestamp_millis(obs.timestamp)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    let embed = DiscordEmbed {
        title: format!("⚠️ HUMBOLDT Alert: {}", rule.name),
        description: Some(format!(
            "Observation layer **{:?}** triggered threshold criteria.",
            rule.layer
        )),
        url: Some(deep_link),
        color,
        fields: vec![
            DiscordEmbedField {
                name: "Observed Reading".to_string(),
                value: format!("**{:.2}** ({})", reading, rule.metric_name),
                inline: true,
            },
            DiscordEmbedField {
                name: "Threshold".to_string(),
                value: format!("{} {:.2}", op_symbol, rule.threshold),
                inline: true,
            },
            DiscordEmbedField {
                name: "Event Location".to_string(),
                value: location_str,
                inline: false,
            },
            DiscordEmbedField {
                name: "Provider Attribution".to_string(),
                value: obs.provider.clone(),
                inline: true,
            },
        ],
        timestamp: Some(iso_timestamp),
        footer: Some(DiscordEmbedFooter {
            text: "HUMBOLDT Planetary Intelligence System".to_string(),
        }),
    };

    DiscordWebhookPayload {
        username: "HUMBOLDT Alerts".to_string(),
        avatar_url: None,
        content: None,
        embeds: vec![embed],
    }
}

/// Dispatches a Discord rich embed alert card to the configured webhook endpoint.
pub async fn send_discord_alert(
    client: &reqwest::Client,
    webhook_url: &str,
    payload: &DiscordWebhookPayload,
) -> Result<()> {
    let res = client.post(webhook_url).json(payload).send().await;
    match res {
        Ok(resp) => {
            if resp.status().is_success() {
                Ok(())
            } else {
                let sanitized = sanitize_webhook_url(webhook_url);
                let status = resp.status();
                let err_msg = format!(
                    "Discord webhook request to {} failed with HTTP {}",
                    sanitized, status
                );
                tracing::warn!("{}", err_msg);
                Err(HumboldtError::Alert(err_msg))
            }
        }
        Err(err) => {
            let sanitized = sanitize_webhook_url(webhook_url);
            let err_msg = format!("Failed to deliver Discord alert to {}: {}", sanitized, err);
            tracing::warn!("{}", err_msg);
            Err(HumboldtError::Alert(err_msg))
        }
    }
}
