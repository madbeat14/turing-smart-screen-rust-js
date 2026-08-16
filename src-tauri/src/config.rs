use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// Top-level config matching config.yaml structure
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct AppConfig {
    pub config: GeneralConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct GeneralConfig {
    #[serde(default = "default_auto")]
    pub com_port: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_auto")]
    pub hw_sensors: String,
    #[serde(default)]
    pub eth: String,
    #[serde(default)]
    pub wlo: String,
    #[serde(default = "default_auto")]
    pub cpu_fan: String,
    #[serde(default = "default_ping")]
    pub ping: String,
    #[serde(default)]
    pub weather_api_key: String,
    #[serde(default = "default_latitude")]
    pub weather_latitude: f64,
    #[serde(default = "default_longitude")]
    pub weather_longitude: f64,
    #[serde(default = "default_metric")]
    pub weather_units: String,
    #[serde(default = "default_en")]
    pub weather_language: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct DisplayConfig {
    #[serde(default = "default_revision_a")]
    pub revision: String,
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default)]
    pub display_reverse: bool,
    #[serde(default = "default_true")]
    pub reset_on_startup: bool,
    /// COM port — populated from GeneralConfig.com_port after loading
    #[serde(skip)]
    pub com_port: String,
}

fn default_auto() -> String {
    "AUTO".to_string()
}
fn default_theme() -> String {
    "v2".to_string()
}
fn default_ping() -> String {
    "8.8.8.8".to_string()
}
fn default_latitude() -> f64 {
    0.0
}
fn default_longitude() -> f64 {
    0.0
}
fn default_metric() -> String {
    "metric".to_string()
}
fn default_en() -> String {
    "en".to_string()
}
fn default_revision_a() -> String {
    "A".to_string()
}
fn default_brightness() -> u8 {
    20
}
fn default_true() -> bool {
    true
}

impl AppConfig {
    /// Load configuration from a path with exhaustive I/O and parse handling.
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::ConfigReadError(format!("{}: {}", path.display(), e)))?;
        let mut config: AppConfig = serde_yaml::from_str(&content)
            .map_err(|e| AppError::ConfigSerializeError(format!("{}: {}", path.display(), e)))?;
        // Copy COM_PORT from general config into display config
        config.display.com_port = config.config.com_port.clone();
        Ok(config)
    }

    /// User data directory — writable location for config, logs, templates, window state.
    /// Uses %APPDATA%\Turing Smart Screen on Windows, falling back to exe-adjacent dir.
    pub fn data_dir() -> std::path::PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return std::path::PathBuf::from(appdata).join("Turing Smart Screen");
            }
        }
        // Fallback: next to the executable (dev builds, non-Windows)
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Ensure the data directory exists. Call once at startup.
    pub fn ensure_data_dir() {
        let dir = Self::data_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Could not create data dir {:?}: {}", dir, e);
        }
    }

    /// Migrate config.yaml from the old exe-adjacent location to data_dir if needed.
    /// Safe to call even if migration is not needed.
    pub fn migrate_from_install_dir() {
        let data = Self::data_dir();
        let new_path = data.join("config.yaml");
        if std::fs::metadata(&new_path).is_ok() {
            return; // Already migrated
        }
        let old_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("config.yaml")));
        if let Some(old) = old_path {
            if std::fs::metadata(&old).is_ok() {
                if let Err(e) = std::fs::copy(&old, &new_path) {
                    log::warn!(
                        "Config migration failed ({} → {}): {}",
                        old.display(),
                        new_path.display(),
                        e
                    );
                } else {
                    log::info!(
                        "Migrated config from {} to {}",
                        old.display(),
                        new_path.display()
                    );
                }
            }
        }
    }

    /// Get the canonical config file path (in data dir).
    pub fn config_path() -> std::path::PathBuf {
        Self::data_dir().join("config.yaml")
    }

    /// Load config from file candidates or return default without TOCTOU races.
    pub fn load_or_default() -> Self {
        let dir = Self::data_dir();
        let candidates = ["config.yaml", "config.yml"];
        for candidate in &candidates {
            let path = dir.join(candidate);
            match std::fs::read_to_string(&path) {
                Ok(content) => match serde_yaml::from_str::<AppConfig>(&content) {
                    Ok(mut config) => {
                        config.display.com_port = config.config.com_port.clone();
                        log::info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse config at {}: {}", path.display(), e);
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Normal when file does not exist yet
                }
                Err(e) => {
                    log::warn!("Failed to read {}: {}", path.display(), e);
                }
            }
        }
        log::info!("Using default configuration");
        Self::default()
    }

    /// Atomically write configuration to disk via temp file + rename.
    pub fn save_atomic(&self, path: &Path) -> Result<(), AppError> {
        self.validate()?;

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::ConfigWriteError(format!("Failed to create config dir: {}", e)))?;

        let yaml = serde_yaml::to_string(self)
            .map_err(|e| AppError::ConfigSerializeError(e.to_string()))?;

        let tmp_path = parent.join(format!(
            ".config_{}.tmp",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)
            .map_err(|e| AppError::ConfigWriteError(format!("Failed to open temp config: {}", e)))?;

        file.write_all(yaml.as_bytes())
            .map_err(|e| AppError::ConfigWriteError(format!("Failed to write temp config: {}", e)))?;
        file.flush()
            .map_err(|e| AppError::ConfigWriteError(format!("Failed to flush temp config: {}", e)))?;
        drop(file);

        std::fs::rename(&tmp_path, path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            AppError::ConfigWriteError(format!("Atomic rename failed: {}", e))
        })?;

        Ok(())
    }

    /// Validate config values against strict high-assurance bounds before saving.
    pub fn validate(&self) -> Result<(), AppError> {
        // COM port: must be "AUTO" or start with "COM" or "/dev/tty"
        let com = &self.config.com_port;
        if com != "AUTO" && !com.starts_with("COM") && !com.starts_with("/dev/tty") {
            return Err(AppError::ValidationError(format!("Invalid COM port: {}", com)));
        }
        if com.is_empty() || com.len() > 64 {
            return Err(AppError::ValidationError("COM port name must be 1-64 characters".into()));
        }

        // Theme: alphanumeric, hyphens, underscores only
        let theme = &self.config.theme;
        if theme.is_empty() || theme.len() > 64 {
            return Err(AppError::ValidationError("Theme name must be 1-64 characters".into()));
        }
        if !theme
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AppError::ValidationError(
                "Theme name contains invalid characters (use alphanumeric, hyphens, underscores)".into()
            ));
        }

        // Weather coordinates: must be finite and within valid geographic bounds
        if !self.config.weather_latitude.is_finite() || !(-90.0..=90.0).contains(&self.config.weather_latitude) {
            return Err(AppError::ValidationError("Weather latitude must be between -90.0 and 90.0".into()));
        }
        if !self.config.weather_longitude.is_finite() || !(-180.0..=180.0).contains(&self.config.weather_longitude) {
            return Err(AppError::ValidationError("Weather longitude must be between -180.0 and 180.0".into()));
        }

        // Weather API key: no control characters, max 256 chars
        if self.config.weather_api_key.len() > 256 || self.config.weather_api_key.chars().any(|c| c.is_control()) {
            return Err(AppError::ValidationError("Weather API key contains invalid characters or exceeds 256 chars".into()));
        }

        // Display brightness: 0 to 100
        if self.display.brightness > 100 {
            return Err(AppError::ValidationError("Display brightness must be between 0 and 100".into()));
        }

        // Display revision: supported hardware types
        const VALID_REVISIONS: &[&str] = &["A", "B", "C", "D", "WEACT_A", "WEACT_B"];
        if !VALID_REVISIONS.contains(&self.display.revision.as_str()) {
            return Err(AppError::ValidationError(format!("Unsupported display revision: {}", self.display.revision)));
        }

        Ok(())
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config: GeneralConfig {
                com_port: default_auto(),
                theme: default_theme(),
                hw_sensors: default_auto(),
                eth: String::new(),
                wlo: String::new(),
                cpu_fan: default_auto(),
                ping: default_ping(),
                weather_api_key: String::new(),
                weather_latitude: default_latitude(),
                weather_longitude: default_longitude(),
                weather_units: default_metric(),
                weather_language: default_en(),
            },
            display: DisplayConfig {
                revision: default_revision_a(),
                brightness: default_brightness(),
                display_reverse: false,
                reset_on_startup: true,
                com_port: default_auto(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_com_port() {
        let mut config = AppConfig::default();
        config.config.com_port = "INVALID_PORT".to_string();
        assert!(config.validate().is_err());

        config.config.com_port = "COM3".to_string();
        assert!(config.validate().is_ok());

        config.config.com_port = "/dev/ttyUSB0".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_theme_name() {
        let mut config = AppConfig::default();
        config.config.theme = "../escape".to_string();
        assert!(config.validate().is_err());

        config.config.theme = "custom-theme_01".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_weather_coordinates() {
        let mut config = AppConfig::default();
        config.config.weather_latitude = f64::NAN;
        assert!(config.validate().is_err());

        config.config.weather_latitude = 120.0; // out of range
        assert!(config.validate().is_err());

        config.config.weather_latitude = 45.0;
        config.config.weather_longitude = 200.0; // out of range
        assert!(config.validate().is_err());

        config.config.weather_longitude = -122.4;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_brightness_and_revision_validation() {
        let mut config = AppConfig::default();
        config.display.brightness = 101;
        assert!(config.validate().is_err());

        config.display.brightness = 100;
        assert!(config.validate().is_ok());

        config.display.revision = "UNKNOWN_REV".to_string();
        assert!(config.validate().is_err());

        config.display.revision = "WEACT_A".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_atomic_save_and_load() {
        let temp_dir = std::env::temp_dir().join(format!("turing_test_config_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let config_file = temp_dir.join("config.yaml");

        let mut config = AppConfig::default();
        config.config.theme = "v1".to_string();
        config.display.brightness = 75;

        assert!(config.save_atomic(&config_file).is_ok());
        let loaded = AppConfig::load(&config_file).expect("Config load must succeed");
        assert_eq!(loaded.config.theme, "v1");
        assert_eq!(loaded.display.brightness, 75);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
