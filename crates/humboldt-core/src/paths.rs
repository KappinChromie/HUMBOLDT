use crate::error::{HumboldtError, Result};
use std::fs;
use std::path::PathBuf;

/// Application path resolution matching HUMBOLDT Architecture Spec Section 3.3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub base_dir: PathBuf,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub cache_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub active_port_file: PathBuf,
}

impl AppPaths {
    /// Resolves standard OS-specific directories for HUMBOLDT, respecting HUMBOLDT_DATA_DIR if set.
    pub fn resolve() -> Result<Self> {
        if let Ok(custom) = std::env::var("HUMBOLDT_DATA_DIR") {
            if !custom.trim().is_empty() {
                let base_dir = PathBuf::from(custom.trim());
                let data_dir = base_dir.join("data");
                let db_path = data_dir.join("humboldt.db");
                let cache_dir = base_dir.join("cache");
                let logs_dir = base_dir.join("logs");
                let active_port_file = base_dir.join(".active_port");

                return Ok(Self {
                    base_dir,
                    data_dir,
                    db_path,
                    cache_dir,
                    logs_dir,
                    active_port_file,
                });
            }
        }

        #[cfg(windows)]
        {
            let appdata = dirs::config_dir().ok_or_else(|| {
                HumboldtError::Config("Unable to resolve APPDATA directory".to_string())
            })?;
            let localappdata = dirs::cache_dir().ok_or_else(|| {
                HumboldtError::Config("Unable to resolve LOCALAPPDATA directory".to_string())
            })?;

            let base_dir = appdata.join("Humboldt");
            let data_dir = base_dir.join("data");
            let db_path = data_dir.join("humboldt.db");
            let cache_dir = localappdata.join("Humboldt").join("cache");
            let logs_dir = localappdata.join("Humboldt").join("logs");
            let active_port_file = base_dir.join(".active_port");

            Ok(Self {
                base_dir,
                data_dir,
                db_path,
                cache_dir,
                logs_dir,
                active_port_file,
            })
        }

        #[cfg(not(windows))]
        {
            let home = dirs::home_dir().ok_or_else(|| {
                HumboldtError::Config("Unable to resolve HOME directory".to_string())
            })?;

            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| home.join(".config"))
                .join("humboldt");
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| home.join(".local").join("share"))
                .join("humboldt")
                .join("data");
            let db_path = data_dir.join("humboldt.db");
            let cache_dir = dirs::cache_dir()
                .unwrap_or_else(|| home.join(".cache"))
                .join("humboldt");
            let logs_dir = dirs::state_dir()
                .unwrap_or_else(|| home.join(".local").join("state"))
                .join("humboldt")
                .join("logs");
            let active_port_file = config_dir.join(".active_port");

            Ok(Self {
                base_dir: config_dir,
                data_dir,
                db_path,
                cache_dir,
                logs_dir,
                active_port_file,
            })
        }
    }

    /// Creates all required runtime directories if they do not yet exist.
    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.cache_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
        Ok(())
    }

    /// Reads the currently active port from the `.active_port` file if present.
    pub fn read_active_port(&self) -> Option<u16> {
        fs::read_to_string(&self.active_port_file)
            .ok()
            .and_then(|s| s.trim().parse::<u16>().ok())
    }

    /// Writes the active port to the `.active_port` file.
    pub fn write_active_port(&self, port: u16) -> Result<()> {
        if let Some(parent) = self.active_port_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.active_port_file, port.to_string())?;
        Ok(())
    }

    /// Removes the `.active_port` file on clean shutdown.
    pub fn remove_active_port(&self) -> Result<()> {
        if self.active_port_file.exists() {
            fs::remove_file(&self.active_port_file)?;
        }
        Ok(())
    }
}
