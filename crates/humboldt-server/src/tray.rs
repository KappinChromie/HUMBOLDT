use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

const MENU_ID_OPEN: &str = "humboldt_open";
const MENU_ID_COPY_URL: &str = "humboldt_copy_url";
const MENU_ID_QUIT: &str = "humboldt_quit";

/// Generates a bundled 32x32 RGBA icon matching Humboldt's diamond glyph in telemetry accent (#58A6FF).
fn create_default_icon() -> Icon {
    const WIDTH: u32 = 32;
    const HEIGHT: u32 = 32;
    let mut rgba = Vec::with_capacity((WIDTH * HEIGHT * 4) as usize);

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let center_x = 15.5f32;
            let center_y = 15.5f32;
            let dx = (x as f32 - center_x).abs();
            let dy = (y as f32 - center_y).abs();
            let dist = dx + dy;

            // Draw diamond outline and center nucleus
            if (9.0..=12.5).contains(&dist) || dist <= 3.0 {
                // #58A6FF (accent telemetry)
                rgba.extend_from_slice(&[88, 166, 255, 255]);
            } else if dist < 9.0 {
                // #161B22 (panel surface)
                rgba.extend_from_slice(&[22, 27, 34, 255]);
            } else {
                // Transparent background
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, WIDTH, HEIGHT).expect("Failed to build tray icon from RGBA buffer")
}

pub struct SystemTrayController {
    is_running: Arc<AtomicBool>,
}

impl SystemTrayController {
    /// Initializes system tray icon on a dedicated UI message loop thread.
    pub fn spawn(port: u16, shutdown_tx: broadcast::Sender<()>) -> Self {
        let is_running = Arc::new(AtomicBool::new(true));
        let is_running_clone = Arc::clone(&is_running);

        std::thread::Builder::new()
            .name("humboldt-tray".to_string())
            .spawn(move || {
                let url = format!("http://127.0.0.1:{}", port);
                let tooltip = format!("HUMBOLDT (Port: {})", port);

                // Build context menu
                let tray_menu = Menu::new();
                let item_open = MenuItem::with_id(MENU_ID_OPEN, "Open Humboldt", true, None);
                let item_copy = MenuItem::with_id(MENU_ID_COPY_URL, "Copy Local URL", true, None);
                let item_quit = MenuItem::with_id(MENU_ID_QUIT, "Quit HUMBOLDT", true, None);

                let _ = tray_menu.append(&item_open);
                let _ = tray_menu.append(&item_copy);
                let _ = tray_menu.append(&PredefinedMenuItem::separator());
                let _ = tray_menu.append(&item_quit);

                let icon = create_default_icon();

                let _tray_icon: Option<TrayIcon> = match TrayIconBuilder::new()
                    .with_menu(Box::new(tray_menu))
                    .with_tooltip(tooltip)
                    .with_icon(icon)
                    .build()
                {
                    Ok(tray) => {
                        info!("System tray icon registered successfully.");
                        Some(tray)
                    }
                    Err(err) => {
                        error!("Failed to create system tray icon: {}", err);
                        None
                    }
                };

                // Event loop for tray and menu interactions
                let menu_channel = MenuEvent::receiver();
                let tray_channel = TrayIconEvent::receiver();

                #[cfg(windows)]
                {
                    use windows_sys::Win32::UI::WindowsAndMessaging::{
                        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
                    };

                    while is_running_clone.load(Ordering::SeqCst) {
                        // Pump Windows messages
                        unsafe {
                            let mut msg: MSG = std::mem::zeroed();
                            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                                TranslateMessage(&msg);
                                DispatchMessageW(&msg);
                            }
                        }

                        // Check menu events
                        if let Ok(event) = menu_channel.try_recv() {
                            if event.id == MENU_ID_OPEN {
                                info!("Tray action: Open Humboldt -> {}", url);
                                let _ = open::that(&url);
                            } else if event.id == MENU_ID_COPY_URL {
                                info!("Tray action: Copy Local URL -> {}", url);
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    if let Err(e) = cb.set_text(url.clone()) {
                                        error!("Failed to copy URL to clipboard: {}", e);
                                    }
                                }
                            } else if event.id == MENU_ID_QUIT {
                                info!("Tray action: Quit HUMBOLDT");
                                let _ = shutdown_tx.send(());
                                is_running_clone.store(false, Ordering::SeqCst);
                                break;
                            }
                        }

                        // Check tray icon events (click to open)
                        if let Ok(TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }) = tray_channel.try_recv()
                        {
                            info!("Tray icon clicked: Opening Humboldt -> {}", url);
                            let _ = open::that(&url);
                        }

                        std::thread::sleep(std::time::Duration::from_millis(16));
                    }
                }

                #[cfg(not(windows))]
                {
                    while is_running_clone.load(Ordering::SeqCst) {
                        if let Ok(event) = menu_channel.try_recv() {
                            if event.id == MENU_ID_OPEN {
                                let _ = open::that(&url);
                            } else if event.id == MENU_ID_COPY_URL {
                                if let Ok(mut cb) = arboard::Clipboard::new() {
                                    let _ = cb.set_text(url.clone());
                                }
                            } else if event.id == MENU_ID_QUIT {
                                let _ = shutdown_tx.send(());
                                break;
                            }
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
            })
            .expect("Failed to spawn tray thread");

        Self { is_running }
    }

    pub fn stop(&self) {
        self.is_running.store(false, Ordering::SeqCst);
    }
}
