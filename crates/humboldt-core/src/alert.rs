use crate::observation::LayerType;
use serde::{Deserialize, Serialize};

/// Comparison operator for alert rule threshold evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonOp {
    GreaterThan,
    LessThan,
    Equal,
}

impl ComparisonOp {
    pub fn evaluate(&self, value: f64, threshold: f64) -> bool {
        match self {
            ComparisonOp::GreaterThan => value > threshold,
            ComparisonOp::LessThan => value < threshold,
            ComparisonOp::Equal => (value - threshold).abs() < f64::EPSILON,
        }
    }
}

/// Alert rule definition matching Architecture Spec Section 10.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRule {
    /// UUIDv4 identifier
    pub id: String,
    /// User-defined label (e.g., 'Major Quake')
    pub name: String,
    pub enabled: bool,
    pub layer: LayerType,
    /// 'magnitude', 'kp_index', 'gauge_height_m'
    pub metric_name: String,
    pub operator: ComparisonOp,
    pub threshold: f64,
    /// Optional spatial polygon filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_boundary_id: Option<String>,
    /// Anti-spam timer in seconds (default: 3600)
    #[serde(default = "default_cooldown")]
    pub cooldown_seconds: u32,
    /// Epoch millis of last dispatch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_triggered: Option<i64>,
    /// Windows Desktop toast notification flag
    #[serde(default = "default_true")]
    pub notify_desktop: bool,
    /// Discord Webhook notification flag
    #[serde(default)]
    pub notify_discord: bool,
}

fn default_cooldown() -> u32 {
    3600
}

fn default_true() -> bool {
    true
}

impl AlertRule {
    /// Evaluates if an observation's value triggers this rule given current time.
    pub fn should_trigger(&self, value: f64, current_time_ms: i64) -> bool {
        if !self.enabled {
            return false;
        }

        if let Some(last) = self.last_triggered {
            let cooldown_ms = (self.cooldown_seconds as i64) * 1000;
            if current_time_ms.saturating_sub(last) < cooldown_ms {
                return false;
            }
        }

        self.operator.evaluate(value, self.threshold)
    }
}
