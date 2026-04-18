use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level config matching config.yaml structure
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct AppConfig {
    pub config: GeneralConfig,
    pub display: DisplayConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Debug, Deserialize, Serialize, Clone)]
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
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AppConfig = serde_yaml::from_str(&content)?;
        // Copy COM_PORT from general config into display config
        config.display.com_port = config.config.com_port.clone();
        Ok(config)
    }

    /// Resolve config directory: next to the executable, not the CWD.
    pub fn config_dir() -> std::path::PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
    }

    /// Get the canonical config file path (next to the executable).
    pub fn config_path() -> std::path::PathBuf {
        Self::config_dir().join("config.yaml")
    }

    pub fn load_or_default() -> Self {
        let dir = Self::config_dir();
        let candidates = ["config.yaml", "config.yml"];
        for candidate in &candidates {
            let path = dir.join(candidate);
            if path.exists() {
                match Self::load(&path) {
                    Ok(config) => {
                        log::info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        log::warn!("Failed to load {}: {}", path.display(), e);
                    }
                }
            }
        }
        log::info!("Using default configuration");
        Self::default()
    }

    /// Validate config values before saving. Returns Err with a description of the problem.
    pub fn validate(&self) -> Result<(), String> {
        // COM port: must be "AUTO" or start with "COM" or "/dev/tty"
        let com = &self.config.com_port;
        if com != "AUTO" && !com.starts_with("COM") && !com.starts_with("/dev/tty") {
            return Err(format!("Invalid COM port: {}", com));
        }
        if com.len() > 64 {
            return Err("COM port name too long".into());
        }

        // Theme: alphanumeric, hyphens, underscores only
        let theme = &self.config.theme;
        if theme.is_empty() || theme.len() > 64 {
            return Err("Theme name must be 1-64 characters".into());
        }
        if !theme
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            return Err(
                "Theme name contains invalid characters (use alphanumeric, hyphens, underscores)"
                    .into(),
            );
        }

        // Weather coordinates: must be finite numbers
        if self.config.weather_latitude.is_nan() || self.config.weather_latitude.is_infinite() {
            return Err("Invalid weather latitude".into());
        }
        if self.config.weather_longitude.is_nan() || self.config.weather_longitude.is_infinite() {
            return Err("Invalid weather longitude".into());
        }

        // Weather API key: no control characters
        if self.config.weather_api_key.chars().any(|c| c.is_control()) {
            return Err("Weather API key contains invalid characters".into());
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
