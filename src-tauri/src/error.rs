//! Typed error taxonomy for Turing Smart Screen
//!
//! Provides exhaustive, serialized error handling across all internal modules
//! and over the Tauri v2 IPC bridge.

use serde::{Serialize, Serializer};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Internal concurrency lock failed: {0}")]
    LockFailed(String),

    #[error("Configuration read failed: {0}")]
    ConfigReadError(String),

    #[error("Configuration serialization failed: {0}")]
    ConfigSerializeError(String),

    #[error("Configuration write failed: {0}")]
    ConfigWriteError(String),

    #[error("Input validation failed: {0}")]
    ValidationError(String),

    #[error("Template '{0}' not found")]
    TemplateNotFound(String),

    #[error("Invalid template name: {0}")]
    InvalidTemplateName(String),

    #[error("Invalid template path: {0}")]
    InvalidTemplatePath(String),

    #[error("Template file I/O error: {0}")]
    TemplateIoError(String),

    #[error("Invalid template manifest JSON: {0}")]
    InvalidManifest(String),

    #[error("Serial port communication error: {0}")]
    SerialError(String),

    #[error("Internal message channel error: {0}")]
    ChannelError(String),

    #[error("Webview window '{0}' was not found")]
    WindowNotFound(String),

    #[error("Windows startup task error: {0}")]
    StartupError(String),

    #[error("Webview script evaluation or navigation failed: {0}")]
    WebviewError(String),

    #[error("System I/O error: {0}")]
    Io(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        Self::InvalidManifest(err.to_string())
    }
}

impl From<serde_yaml::Error> for AppError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::ConfigSerializeError(err.to_string())
    }
}

impl From<serialport::Error> for AppError {
    fn from(err: serialport::Error) -> Self {
        Self::SerialError(err.to_string())
    }
}
