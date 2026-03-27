/// Template management for the editor.
///
/// Provides Tauri commands for listing, reading, saving, deleting,
/// and cloning templates. Custom templates live next to the executable
/// in a `templates/` directory. Built-in templates (v1, v2) are
/// bundled with the app and are read-only.

use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Built-in template names that cannot be overwritten or deleted.
const BUILTIN_TEMPLATES: &[&str] = &["v1", "v2"];

/// Information about a single template.
#[derive(Debug, Serialize, Clone)]
pub struct TemplateInfo {
    pub name: String,
    pub display_name: String,
    pub has_manifest: bool,
    pub is_builtin: bool,
}

/// Returns the directory where user-created templates are stored (next to the exe).
fn user_templates_dir() -> PathBuf {
    crate::config::AppConfig::config_dir().join("templates")
}

/// Returns the bundled templates directory (inside `src/templates/` during dev).
fn bundled_templates_dir() -> PathBuf {
    // In dev mode, the frontend dist is `../src`, so templates are at `../src/templates`
    // In production, Tauri serves from the bundled frontend dist
    let exe_dir = crate::config::AppConfig::config_dir();
    exe_dir.join("templates")
}

/// Validate a template name: lowercase alphanumeric + hyphens/underscores, 1-64 chars.
fn validate_template_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("Template name must be 1-64 characters".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(
            "Template name must contain only lowercase letters, digits, hyphens, underscores"
                .into(),
        );
    }
    // Prevent path traversal
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err("Template name contains invalid characters".into());
    }
    Ok(())
}

/// Check if a name is a built-in template.
fn is_builtin(name: &str) -> bool {
    BUILTIN_TEMPLATES.contains(&name)
}

/// Resolve the path to a template directory, checking both user and bundled locations.
/// User templates take priority over bundled ones.
fn resolve_template_dir(name: &str) -> Option<PathBuf> {
    let user_path = user_templates_dir().join(name);
    if user_path.exists() && user_path.is_dir() {
        return Some(user_path);
    }
    let bundled_path = bundled_templates_dir().join(name);
    if bundled_path.exists() && bundled_path.is_dir() {
        return Some(bundled_path);
    }
    None
}

/// List all available templates (built-in + user-created).
#[tauri::command]
pub fn list_templates() -> Result<Vec<TemplateInfo>, String> {
    let mut templates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan user templates directory
    let user_dir = user_templates_dir();
    if user_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&user_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let name = name.to_string();
                        if validate_template_name(&name).is_ok() {
                            let has_manifest = path.join("manifest.json").exists();
                            let display_name = read_display_name(&path).unwrap_or_else(|| {
                                format_display_name(&name)
                            });
                            templates.push(TemplateInfo {
                                name: name.clone(),
                                display_name,
                                has_manifest,
                                is_builtin: is_builtin(&name),
                            });
                            seen.insert(name);
                        }
                    }
                }
            }
        }
    }

    // Add built-in templates that aren't already in user dir
    for builtin in BUILTIN_TEMPLATES {
        if !seen.contains(*builtin) {
            templates.push(TemplateInfo {
                name: builtin.to_string(),
                display_name: format_display_name(builtin),
                has_manifest: false,
                is_builtin: true,
            });
        }
    }

    // Sort: built-in first, then alphabetical
    templates.sort_by(|a, b| {
        b.is_builtin
            .cmp(&a.is_builtin)
            .then(a.name.cmp(&b.name))
    });

    Ok(templates)
}

/// Read the display name from a manifest.json if it exists.
fn read_display_name(template_dir: &std::path::Path) -> Option<String> {
    let manifest_path = template_dir.join("manifest.json");
    if manifest_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                return json
                    .get("displayName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
        }
    }
    None
}

/// Format a template name into a display name (e.g., "my-template" -> "My Template").
fn format_display_name(name: &str) -> String {
    name.split(|c: char| c == '-' || c == '_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    upper + &chars.collect::<String>()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read a template's manifest.json.
#[tauri::command]
pub fn read_template_manifest(name: String) -> Result<String, String> {
    validate_template_name(&name)?;

    let dir = resolve_template_dir(&name)
        .ok_or_else(|| format!("Template '{}' not found", name))?;

    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(format!("Template '{}' has no manifest (not editable)", name));
    }

    std::fs::read_to_string(&manifest_path).map_err(|e| {
        error!("Failed to read manifest for '{}': {}", name, e);
        format!("Failed to read manifest: {}", e)
    })
}

/// Save template files (manifest.json, template.html, app.js, style.css).
/// Writes atomically by writing to temp files first, then renaming.
#[derive(Deserialize)]
pub struct SaveTemplateArgs {
    pub name: String,
    pub manifest: String,
    pub html: String,
    pub css: String,
    pub js: String,
}

#[tauri::command]
pub fn save_template(args: SaveTemplateArgs) -> Result<(), String> {
    validate_template_name(&args.name)?;

    if is_builtin(&args.name) {
        return Err(format!(
            "Cannot overwrite built-in template '{}'. Clone it first.",
            args.name
        ));
    }

    // Validate manifest JSON is parseable
    serde_json::from_str::<serde_json::Value>(&args.manifest)
        .map_err(|e| format!("Invalid manifest JSON: {}", e))?;

    let dir = user_templates_dir().join(&args.name);

    // Ensure directory exists
    std::fs::create_dir_all(&dir).map_err(|e| {
        error!("Failed to create template dir '{}': {}", dir.display(), e);
        format!("Failed to create template directory: {}", e)
    })?;

    // Verify the resolved path is within the templates directory (path traversal prevention)
    let canonical_dir = dir.canonicalize().unwrap_or_else(|_| dir.clone());
    let canonical_templates = user_templates_dir()
        .canonicalize()
        .unwrap_or_else(|_| user_templates_dir());
    if !canonical_dir.starts_with(&canonical_templates) {
        return Err("Invalid template path".into());
    }

    // Write each file atomically (write to .tmp, then rename)
    let files = [
        ("manifest.json", &args.manifest),
        ("template.html", &args.html),
        ("style.css", &args.css),
        ("app.js", &args.js),
    ];

    for (filename, content) in &files {
        let target = dir.join(filename);
        let tmp = dir.join(format!("{}.tmp", filename));

        std::fs::write(&tmp, content).map_err(|e| {
            error!("Failed to write {}: {}", tmp.display(), e);
            format!("Failed to write {}: {}", filename, e)
        })?;

        std::fs::rename(&tmp, &target).map_err(|e| {
            error!("Failed to rename {} -> {}: {}", tmp.display(), target.display(), e);
            // Clean up temp file on failure
            let _ = std::fs::remove_file(&tmp);
            format!("Failed to save {}: {}", filename, e)
        })?;
    }

    info!("Template '{}' saved to {}", args.name, dir.display());
    Ok(())
}

/// Delete a custom template. Refuses to delete built-in templates.
#[tauri::command]
pub fn delete_template(name: String) -> Result<(), String> {
    validate_template_name(&name)?;

    if is_builtin(&name) {
        return Err(format!("Cannot delete built-in template '{}'", name));
    }

    let dir = user_templates_dir().join(&name);
    if !dir.exists() {
        return Err(format!("Template '{}' not found", name));
    }

    // Verify path is within templates directory
    let canonical_dir = dir.canonicalize().map_err(|e| format!("Path error: {}", e))?;
    let canonical_templates = user_templates_dir()
        .canonicalize()
        .unwrap_or_else(|_| user_templates_dir());
    if !canonical_dir.starts_with(&canonical_templates) {
        return Err("Invalid template path".into());
    }

    std::fs::remove_dir_all(&dir).map_err(|e| {
        error!("Failed to delete template '{}': {}", name, e);
        format!("Failed to delete template: {}", e)
    })?;

    info!("Template '{}' deleted", name);
    Ok(())
}

/// Clone a template to a new name.
#[tauri::command]
pub fn clone_template(source: String, target: String) -> Result<(), String> {
    validate_template_name(&source)?;
    validate_template_name(&target)?;

    if is_builtin(&target) {
        return Err(format!("Cannot use built-in name '{}' as target", target));
    }

    let source_dir = resolve_template_dir(&source)
        .ok_or_else(|| format!("Source template '{}' not found", source))?;

    let target_dir = user_templates_dir().join(&target);
    if target_dir.exists() {
        return Err(format!("Template '{}' already exists", target));
    }

    // Create target directory
    std::fs::create_dir_all(&target_dir).map_err(|e| {
        format!("Failed to create directory: {}", e)
    })?;

    // Copy all files from source to target
    if let Ok(entries) = std::fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let dest = target_dir.join(filename);
                    std::fs::copy(&path, &dest).map_err(|e| {
                        format!("Failed to copy {}: {}", path.display(), e)
                    })?;
                }
            }
        }
    }

    info!("Template '{}' cloned to '{}'", source, target);
    Ok(())
}
