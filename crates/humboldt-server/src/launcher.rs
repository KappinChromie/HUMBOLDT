use tracing::{error, info};

/// Returns true if running in a headless environment without GUI display.
pub fn is_headless() -> bool {
    if std::env::var("HUMBOLDT_HEADLESS").map(|v| v == "1" || v == "true").unwrap_or(false) {
        return true;
    }
    if std::env::var("DOCKER").is_ok() || std::path::Path::new("/.dockerenv").exists() {
        return true;
    }

    #[cfg(not(windows))]
    {
        let display = std::env::var("DISPLAY").unwrap_or_default();
        let wayland = std::env::var("WAYLAND_DISPLAY").unwrap_or_default();
        display.is_empty() && wayland.is_empty()
    }

    #[cfg(windows)]
    {
        false
    }
}

/// Asynchronously dispatches default web browser to the local HUMBOLDT instance URL.
pub fn launch_browser(port: u16) {
    if is_headless() {
        info!("Headless mode detected; skipping default browser launch.");
        return;
    }

    let url = format!("http://127.0.0.1:{}", port);
    tokio::spawn(async move {
        // Small delay to ensure Axum socket is accepting incoming TCP connections
        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
        info!("Launching system default browser to {}", url);
        if let Err(err) = open::that(&url) {
            error!("Failed to open system browser: {}", err);
        }
    });
}
