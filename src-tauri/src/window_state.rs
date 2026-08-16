/// Persists window state (size, panel widths, etc.) across sessions.
/// Stores a JSON file in AppData: window_state.json
///
/// Each window label maps to a JSON object so the frontend
/// can save and restore geometry across launches.
use log::{error, info};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

type StateMap = HashMap<String, Value>;

fn state_path() -> PathBuf {
    crate::config::AppConfig::data_dir().join("window_state.json")
}

fn load_all() -> StateMap {
    let path = state_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            error!("Failed to read window state file {:?}: {}", path, e);
            HashMap::new()
        }
    }
}

fn save_all(states: &StateMap) {
    let path = state_path();
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    if let Err(e) = std::fs::create_dir_all(parent) {
        error!("Failed to create window state directory {:?}: {}", parent, e);
        return;
    }

    match serde_json::to_string_pretty(states) {
        Ok(json) => {
            let tmp_path = parent.join(format!(
                ".window_state_{}.tmp",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));

            let write_result = (|| -> std::io::Result<()> {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&tmp_path)?;
                file.write_all(json.as_bytes())?;
                file.flush()?;
                drop(file);
                std::fs::rename(&tmp_path, &path)?;
                Ok(())
            })();

            if let Err(e) = write_result {
                let _ = std::fs::remove_file(&tmp_path);
                error!("Failed to atomically save window state: {}", e);
            }
        }
        Err(e) => error!("Failed to serialize window state: {}", e),
    }
}

/// Get saved state for a window. Returns the JSON object or null.
#[tauri::command]
pub fn get_window_state(label: String) -> Value {
    if label.len() > 64 || label.contains('/') || label.contains('\\') || label.contains("..") {
        return Value::Null;
    }
    let states = load_all();
    states.get(&label).cloned().unwrap_or(Value::Null)
}

/// Save state for a window. Merges the provided object into existing state.
#[tauri::command]
pub fn save_window_state(label: String, state: Value) {
    if label.len() > 64 || label.contains('/') || label.contains('\\') || label.contains("..") {
        error!("Refusing to save window state with invalid label: {}", label);
        return;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_window_state() {
        let mut base_map = StateMap::new();
        let initial = serde_json::json!({
            "width": 800,
            "height": 600
        });
        base_map.insert("editor".into(), initial);

        let incoming = serde_json::json!({
            "height": 650,
            "leftPanelWidth": 220
        });

        let merged = match (base_map.get("editor").cloned(), &incoming) {
            (Some(Value::Object(mut existing)), Value::Object(inc)) => {
                for (k, v) in inc {
                    existing.insert(k.clone(), v.clone());
                }
                Value::Object(existing)
            }
            _ => incoming,
        };

        assert_eq!(merged["width"], 800);
        assert_eq!(merged["height"], 650);
        assert_eq!(merged["leftPanelWidth"], 220);
    }
}
