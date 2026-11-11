use serde::{Deserialize, Serialize};

pub const DEFAULT_PORT_START: u16 = 8374;
pub const DEFAULT_PORT_END: u16 = 8378;
pub const CANONICAL_USER_AGENT: &str = "HUMBOLDT/1.0 (+https://github.com/humboldt/humboldt)";
pub const WIN32_MUTEX_NAME: &str = "Global\\HumboldtAppMutex";

/// Global application configuration settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub bind_address: String,
    pub port_range_start: u16,
    pub port_range_end: u16,
    pub lan_mode: bool,
    pub disk_cap_gb: f64,
}

impl Default for AppConfig {
    fn default() -> Self {
        let bind_address = std::env::var("HUMBOLDT_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port_range_start = std::env::var("HUMBOLDT_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT_START);
        let port_range_end = std::env::var("HUMBOLDT_PORT_END")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(DEFAULT_PORT_END);
        let lan_mode = std::env::var("HUMBOLDT_LAN_MODE")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false);

        Self {
            bind_address,
            port_range_start,
            port_range_end,
            lan_mode,
            disk_cap_gb: 10.0,
        }
    }
}

impl AppConfig {
    /// Returns the active bind address taking into account the LAN mode flag or environment variables.
    pub fn resolved_bind_address(&self) -> &str {
        if self.lan_mode {
            "0.0.0.0"
        } else {
            &self.bind_address
        }
    }
}
