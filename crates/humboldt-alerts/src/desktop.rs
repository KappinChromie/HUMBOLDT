use humboldt_core::Result;

#[cfg(target_os = "windows")]
use winrt_notification::Toast;

/// Dispatches a native Win32 Toast notification registered under AUMID 'Humboldt.Desktop'.
#[cfg(target_os = "windows")]
pub fn send_desktop_notification(title: &str, message: &str) -> Result<()> {
    // AUMID specified in Section 10.2: Humboldt.Desktop
    if let Err(e) = Toast::new("Humboldt.Desktop")
        .title(title)
        .text1(message)
        .show()
    {
        tracing::warn!("Failed to dispatch Windows toast: {}", e);
    }
    Ok(())
}

/// Headless / non-Windows fallback.
#[cfg(not(target_os = "windows"))]
pub fn send_desktop_notification(title: &str, message: &str) -> Result<()> {
    tracing::info!("[HEADLESS NOTIFICATION] {}: {}", title, message);
    Ok(())
}
