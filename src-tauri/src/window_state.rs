/// Persists window state (size, panel widths, etc.) across sessions.
/// Stores a JSON file next to the executable: window_state.json
///
/// Each window label maps to an arbitrary JSON object so the frontend
/// can save whatever it needs (width, height, panel widths, etc.).

use log::{error, info};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

type StateMap = HashMap<String, Value>;

fn state_path() -> PathBuf {
    crate::config::AppConfig::config_dir().join("window_state.json")
}

fn load_all() -> StateMap {
    let path = state_path();
    if !path.exists() {
        return HashMap::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

fn save_all(states: &StateMap) {
    let path = state_path();
    match serde_json::to_string_pretty(states) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                error!("Failed to write window state: {}", e);
            }
        }
        Err(e) => error!("Failed to serialize window state: {}", e),
    }
}

/// Get saved state for a window. Returns the JSON object or null.
#[tauri::command]
pub fn get_window_state(label: String) -> Value {
    let states = load_all();
    states.get(&label).cloned().unwrap_or(Value::Null)
}

/// Save state for a window. Merges the provided object into existing state.
#[tauri::command]
pub fn save_window_state(label: String, state: Value) {
    let mut states = load_all();

    // Merge: if both existing and new are objects, merge keys; otherwise replace
    let merged = match (states.get(&label).cloned(), &state) {
        (Some(Value::Object(mut existing)), Value::Object(incoming)) => {
            for (k, v) in incoming {
                existing.insert(k.clone(), v.clone());
            }
            Value::Object(existing)
        }
        _ => state,
    };

    states.insert(label.clone(), merged);
    save_all(&states);
    info!("Window state saved for '{}'", label);
}
