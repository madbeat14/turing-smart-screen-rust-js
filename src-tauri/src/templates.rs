/// Template management for the editor.
///
/// Provides Tauri commands for listing, reading, saving, deleting,
/// and cloning templates. Custom templates live in AppData `templates/`.
/// Built-in templates (v1, v2) are bundled with the app and are read-only.
use crate::error::AppError;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use tauri::Manager;

/// Built-in template names that cannot be overwritten or deleted.
const BUILTIN_TEMPLATES: &[&str] = &["v1", "v2"];

/// Information about a single template.
#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct TemplateInfo {
    pub name: String,
    pub display_name: String,
    pub has_manifest: bool,
    pub is_builtin: bool,
    /// True when a built-in template has been copied to user dir (editable, can be reset).
    pub is_modified: bool,
}

/// Returns the directory where user-created templates are stored (in AppData).
fn user_templates_dir() -> PathBuf {
    crate::config::AppConfig::data_dir().join("templates")
}

/// Ensures the user templates base directory exists on disk and returns its canonical path.
fn ensure_user_templates_dir() -> Result<PathBuf, AppError> {
    let dir = user_templates_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| AppError::TemplateIoError(format!("Failed to create templates dir {:?}: {}", dir, e)))?;
    dir.canonicalize()
        .map_err(|e| AppError::TemplateIoError(format!("Failed to canonicalize templates dir {:?}: {}", dir, e)))
}

/// Returns all candidate directories where built-in templates might live.
fn builtin_template_candidates(resource_dir: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    // Production: templates bundled as resources in install dir
    candidates.push(resource_dir.join("templates"));

    // Dev mode: exe is in src-tauri/target/{debug|release}/
    if let Some(target_dir) = resource_dir.parent() {
        if let Some(src_tauri_dir) = target_dir.parent() {
            if let Some(project_root) = src_tauri_dir.parent() {
                candidates.push(project_root.join("src").join("templates"));
            }
        }
    }

    candidates
}

/// Validate a template name: lowercase alphanumeric + hyphens/underscores, 1-64 chars.
pub fn validate_template_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::InvalidTemplateName("Template name must be 1-64 characters".into()));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(AppError::InvalidTemplateName(
            "Template name must contain only lowercase letters, digits, hyphens, underscores".into()
        ));
    }
    // Prevent path traversal
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(AppError::InvalidTemplateName("Template name contains invalid path traversal characters".into()));
    }
    Ok(())
}

/// Check if a name is a built-in template.
pub fn is_builtin(name: &str) -> bool {
    BUILTIN_TEMPLATES.contains(&name)
}

/// Resolve the path to a template directory, checking user dir then built-in locations.
/// User templates take priority over bundled ones.
fn resolve_template_dir(name: &str, resource_dir: &Path) -> Option<PathBuf> {
    // Check user templates first
    let user_path = user_templates_dir().join(name);
    if user_path.is_dir() {
        return Some(user_path);
    }
    // Check all built-in candidate directories
    for candidate in builtin_template_candidates(resource_dir) {
        let path = candidate.join(name);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

/// Get resource_dir from AppHandle, falling back to exe-adjacent dir.
fn get_resource_dir(app: &tauri::AppHandle) -> PathBuf {
    app.path().resource_dir().unwrap_or_else(|_| {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
    })
}

/// List all available templates (built-in + user-created).
#[tauri::command]
pub fn list_templates(app: tauri::AppHandle) -> Result<Vec<TemplateInfo>, AppError> {
    let _resource_dir = get_resource_dir(&app);
    let mut templates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan user templates directory
    let user_dir = user_templates_dir();
    match std::fs::read_dir(&user_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        let name = name.to_string();
                        if validate_template_name(&name).is_ok() {
                            let has_manifest = path.join("manifest.json").is_file();
                            let builtin = is_builtin(&name);
                            let display_name = read_display_name(&path)
                                .unwrap_or_else(|| format_display_name(&name));
                            templates.push(TemplateInfo {
                                name: name.clone(),
                                display_name,
                                has_manifest,
                                is_builtin: builtin,
                                is_modified: builtin, // user-dir copy of a builtin = modified
                            });
                            seen.insert(name);
                        }
                    }
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Normal when no user templates exist yet
        }
        Err(e) => {
            error!("Failed to read user templates directory {:?}: {}", user_dir, e);
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
                is_modified: false,
            });
        }
    }

    // Sort: built-in first, then alphabetical
    templates.sort_by(|a, b| b.is_builtin.cmp(&a.is_builtin).then(a.name.cmp(&b.name)));

    Ok(templates)
}

/// Read the display name from a manifest.json if it exists.
fn read_display_name(template_dir: &Path) -> Option<String> {
    let manifest_path = template_dir.join("manifest.json");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            return json
                .get("displayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
    }
    None
}

/// Format a template name into a display name (e.g., "my-template" -> "My Template").
pub fn format_display_name(name: &str) -> String {
    name.split(['-', '_'])
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
pub fn read_template_manifest(name: String, app: tauri::AppHandle) -> Result<String, AppError> {
    validate_template_name(&name)?;
    let resource_dir = get_resource_dir(&app);
    let dir = resolve_template_dir(&name, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(name.clone()))?;

    let manifest_path = dir.join("manifest.json");
    match std::fs::read_to_string(&manifest_path) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(AppError::InvalidManifest(format!("Template '{}' has no manifest (not editable)", name)))
        }
        Err(e) => Err(AppError::TemplateIoError(format!("Failed to read manifest for '{}': {}", name, e))),
    }
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
pub fn save_template(args: SaveTemplateArgs) -> Result<(), AppError> {
    validate_template_name(&args.name)?;

    // Validate manifest JSON is parseable
    serde_json::from_str::<serde_json::Value>(&args.manifest)
        .map_err(|e| AppError::InvalidManifest(format!("Invalid manifest JSON: {}", e)))?;

    let base_dir = ensure_user_templates_dir()?;
    let dir = base_dir.join(&args.name);

    std::fs::create_dir_all(&dir).map_err(|e| {
        error!("Failed to create template dir '{}': {}", dir.display(), e);
        AppError::TemplateIoError(format!("Failed to create template directory: {}", e))
    })?;

    // Verify the resolved path is strictly within the templates directory (path traversal prevention)
    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| AppError::InvalidTemplatePath(format!("Failed to resolve template path: {}", e)))?;
    if !canonical_dir.starts_with(&base_dir) {
        return Err(AppError::InvalidTemplatePath("Invalid template path (traversal blocked)".into()));
    }

    // Write each file atomically (write to .tmp, then rename)
    let files = [
        ("manifest.json", &args.manifest),
        ("template.html", &args.html),
        ("style.css", &args.css),
        ("app.js", &args.js),
    ];

    for (filename, content) in &files {
        let target = canonical_dir.join(filename);
        let tmp = canonical_dir.join(format!(
            ".{}_{}.tmp",
            filename,
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()
        ));

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)
            .map_err(|e| AppError::TemplateIoError(format!("Failed to open temp {}: {}", filename, e)))?;

        file.write_all(content.as_bytes())
            .map_err(|e| AppError::TemplateIoError(format!("Failed to write {}: {}", filename, e)))?;
        file.flush()
            .map_err(|e| AppError::TemplateIoError(format!("Failed to flush {}: {}", filename, e)))?;
        drop(file);

        std::fs::rename(&tmp, &target).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            error!("Failed to rename {} -> {}: {}", tmp.display(), target.display(), e);
            AppError::TemplateIoError(format!("Failed to save {}: {}", filename, e))
        })?;
    }

    info!("Template '{}' saved securely to {}", args.name, canonical_dir.display());
    Ok(())
}

/// Delete a custom template. Refuses to delete built-in templates.
#[tauri::command]
pub fn delete_template(name: String) -> Result<(), AppError> {
    validate_template_name(&name)?;

    if is_builtin(&name) {
        return Err(AppError::ValidationError(format!("Cannot delete built-in template '{}'", name)));
    }

    let base_dir = ensure_user_templates_dir()?;
    let dir = base_dir.join(&name);

    if !dir.is_dir() {
        return Err(AppError::TemplateNotFound(format!("Template '{}' not found", name)));
    }

    let canonical_dir = dir
        .canonicalize()
        .map_err(|e| AppError::InvalidTemplatePath(format!("Failed to resolve template path: {}", e)))?;
    if !canonical_dir.starts_with(&base_dir) {
        return Err(AppError::InvalidTemplatePath("Invalid template path".into()));
    }

    std::fs::remove_dir_all(&canonical_dir).map_err(|e| {
        error!("Failed to delete template '{}': {}", name, e);
        AppError::TemplateIoError(format!("Failed to delete template: {}", e))
    })?;

    info!("Template '{}' deleted", name);
    Ok(())
}

/// Return true if a user-dir template with this name exists (has manifest.json).
#[tauri::command]
pub fn user_template_exists(name: String) -> bool {
    if validate_template_name(&name).is_err() {
        return false;
    }
    let path = user_templates_dir().join(&name).join("manifest.json");
    path.is_file()
}

/// Clone a template to a new name.
#[tauri::command]
pub fn clone_template(source: String, target: String, app: tauri::AppHandle) -> Result<(), AppError> {
    validate_template_name(&source)?;
    validate_template_name(&target)?;

    if is_builtin(&target) {
        return Err(AppError::ValidationError(format!("Cannot use built-in name '{}' as clone target", target)));
    }

    let resource_dir = get_resource_dir(&app);
    let source_dir = resolve_template_dir(&source, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(format!("Source template '{}' not found", source)))?;

    let base_dir = ensure_user_templates_dir()?;
    let target_dir = base_dir.join(&target);
    if target_dir.exists() {
        return Err(AppError::ValidationError(format!("Template '{}' already exists", target)));
    }

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| AppError::TemplateIoError(format!("Failed to create directory: {}", e)))?;

    // Copy all files from source to target
    if let Ok(entries) = std::fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let dest = target_dir.join(filename);
                    std::fs::copy(&path, &dest)
                        .map_err(|e| AppError::TemplateIoError(format!("Failed to copy {}: {}", path.display(), e)))?;
                }
            }
        }
    }

    // Generate default manifest if absent
    let manifest_path = target_dir.join("manifest.json");
    if !manifest_path.is_file() {
        let default_manifest = generate_default_manifest(&source, &target);
        std::fs::write(&manifest_path, &default_manifest)
            .map_err(|e| AppError::TemplateIoError(format!("Failed to write manifest: {}", e)))?;
        info!("Generated default manifest for cloned template '{}'", target);
    }

    info!("Template '{}' cloned to '{}'", source, target);
    Ok(())
}

/// Generate a default manifest.json that matches the standard 6-card layout.
pub fn generate_default_manifest(source_name: &str, target_name: &str) -> String {
    let display_name = format_display_name(target_name);
    let use_sparklines = source_name != "v1";

    let (primary_fs, secondary_fs, header_fs) = if source_name == "v1" {
        (22, 12, 10)
    } else {
        (20, 11, 9)
    };

    let widget_style = serde_json::json!({
        "backgroundColor": "#16161e",
        "borderColor": "rgba(255,255,255,0.06)",
        "borderRadius": 8,
        "fontFamily": "'Inter', 'Segoe UI', system-ui, sans-serif",
        "primaryFontSize": primary_fs,
        "secondaryFontSize": secondary_fs,
        "headerFontSize": header_fs,
        "textColor": "#e6edf3",
        "secondaryTextColor": "#6b7280",
        "normalColor": "#22c55e",
        "warningColor": "#f59e0b",
        "criticalColor": "#ef4444"
    });

    let theme = if source_name == "v1" { "v1" } else { "v2" };

    let manifest = serde_json::json!({
        "version": 1,
        "name": target_name,
        "displayName": display_name,
        "author": "",
        "description": format!("Cloned from {}", source_name),
        "canvasWidth": 480,
        "canvasHeight": 320,
        "backgroundColor": "#0c0c10",
        "widgets": [
            {
                "id": "w-cpu",
                "type": "metric-card",
                "x": 5, "y": 5, "width": 155, "height": 155,
                "config": {
                    "title": "CPU", "icon": "cpu", "theme": theme,
                    "primaryField": "cpu_temp", "primaryUnit": "\u{00b0}C",
                    "secondaryFields": [
                        {"field": "cpu_freq", "unit": "MHz"},
                        {"field": "cpu_usage", "unit": "%"}
                    ],
                    "progressField": "cpu_usage", "sparklineField": "cpu_usage",
                    "showSparkline": use_sparklines, "showProgress": true,
                    "thresholds": {"warning": 60, "critical": 80}
                },
                "style": widget_style.clone()
            },
            {
                "id": "w-gpu",
                "type": "metric-card",
                "x": 165, "y": 5, "width": 155, "height": 155,
                "config": {
                    "title": "GPU", "icon": "gpu", "theme": theme,
                    "primaryField": "gpu_temp", "primaryUnit": "\u{00b0}C",
                    "secondaryFields": [
                        {"field": "gpu_freq", "unit": "MHz"},
                        {"field": "gpu_usage", "unit": "%"}
                    ],
                    "progressField": "gpu_usage", "sparklineField": "gpu_usage",
                    "showSparkline": use_sparklines, "showProgress": true,
                    "thresholds": {"warning": 60, "critical": 80}
                },
                "style": widget_style.clone()
            },
            {
                "id": "w-mem",
                "type": "metric-card",
                "x": 325, "y": 5, "width": 150, "height": 155,
                "config": {
                    "title": "MEMORY", "icon": "memory", "theme": theme,
                    "primaryField": "ram_used", "primaryUnit": "%",
                    "secondaryFields": [
                        {"field": "ram_total", "unit": ""}
                    ],
                    "progressField": "ram_usage", "sparklineField": "ram_usage",
                    "showSparkline": use_sparklines, "showProgress": true,
                    "thresholds": {"warning": 70, "critical": 90}
                },
                "style": widget_style.clone()
            },
            {
                "id": "w-disk",
                "type": "metric-card",
                "x": 5, "y": 165, "width": 155, "height": 150,
                "config": {
                    "title": "DISK", "icon": "disk", "theme": theme,
                    "primaryField": "disk_used", "primaryUnit": "%",
                    "secondaryFields": [
                         {"field": "disk_total", "unit": ""}
                    ],
                    "progressField": "disk_usage", "sparklineField": "disk_usage",
                    "showSparkline": use_sparklines, "showProgress": true,
                    "thresholds": {"warning": 80, "critical": 95}
                },
                "style": widget_style.clone()
            },
            {
                "id": "w-net",
                "type": "network-pair",
                "x": 165, "y": 165, "width": 155, "height": 150,
                "config": {
                    "theme": theme,
                    "showSparkline": use_sparklines,
                    "maxPoints": 120
                },
                "style": widget_style.clone()
            },
            {
                "id": "w-clock",
                "type": "clock",
                "x": 325, "y": 165, "width": 150, "height": 150,
                "config": {
                    "theme": theme,
                    "format24h": true,
                    "showDate": true,
                    "showSeconds": true
                },
                "style": widget_style
            }
        ]
    });

    serde_json::to_string_pretty(&manifest).unwrap_or_else(|_| "{}".to_string())
}

#[derive(Serialize)]
pub struct TemplateFiles {
    pub html: String,
    pub css: String,
    pub js: String,
}

#[tauri::command]
pub fn read_template_files(name: String, app: tauri::AppHandle) -> Result<TemplateFiles, AppError> {
    validate_template_name(&name)?;
    let resource_dir = get_resource_dir(&app);
    let dir = resolve_template_dir(&name, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(name.clone()))?;

    let read_file = |filename: &str| -> Result<String, AppError> {
        const ALLOWED: &[&str] = &["template.html", "style.css", "app.js"];
        if !ALLOWED.contains(&filename) {
            return Err(AppError::InvalidTemplatePath(format!("Forbidden file name: {}", filename)));
        }
        let path = dir.join(filename);
        std::fs::read_to_string(&path)
            .map_err(|e| AppError::TemplateIoError(format!("Failed to read {}/{}: {}", name, filename, e)))
    };

    Ok(TemplateFiles {
        html: read_file("template.html")?,
        css: read_file("style.css")?,
        js: read_file("app.js")?,
    })
}

#[derive(Serialize)]
pub struct TemplatePaths {
    pub css_path: String,
    pub js_path: String,
}

#[tauri::command]
pub fn get_template_paths(name: String, app: tauri::AppHandle) -> Result<TemplatePaths, AppError> {
    validate_template_name(&name)?;
    let resource_dir = get_resource_dir(&app);
    let dir = resolve_template_dir(&name, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(name.clone()))?;

    let css_path = dir.join("style.css");
    let js_path = dir.join("app.js");

    if !css_path.is_file() {
        return Err(AppError::TemplateNotFound(format!("Template '{}' missing style.css", name)));
    }
    if !js_path.is_file() {
        return Err(AppError::TemplateNotFound(format!("Template '{}' missing app.js", name)));
    }

    Ok(TemplatePaths {
        css_path: css_path.to_string_lossy().to_string(),
        js_path: js_path.to_string_lossy().to_string(),
    })
}

#[tauri::command]
pub fn inject_custom_template(name: String, app: tauri::AppHandle) -> Result<(), AppError> {
    validate_template_name(&name)?;
    let resource_dir = get_resource_dir(&app);
    let dir = resolve_template_dir(&name, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(name.clone()))?;

    let css = std::fs::read_to_string(dir.join("style.css"))
        .map_err(|e| AppError::TemplateIoError(format!("Failed to read style.css: {}", e)))?;
    let js = std::fs::read_to_string(dir.join("app.js"))
        .map_err(|e| AppError::TemplateIoError(format!("Failed to read app.js: {}", e)))?;

    let monitor = app
        .get_webview_window("monitor")
        .ok_or_else(|| AppError::WindowNotFound("monitor".into()))?;

    let css_escaped = css
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");
    let css_js = format!(
        r#"(function() {{
            if (typeof wlog === 'function') wlog('[TEMPLATE] Injecting CSS via adoptedStyleSheets...');
            var sheet = new CSSStyleSheet();
            sheet.replaceSync(`{}`);
            document.adoptedStyleSheets = [sheet];
            if (typeof wlog === 'function') wlog('[TEMPLATE] CSS adoptedStyleSheets applied');
        }})();"#,
        css_escaped
    );
    monitor
        .eval(&css_js)
        .map_err(|e| AppError::WebviewError(format!("CSS inject failed: {}", e)))?;
    info!("[inject_custom_template] CSS injected for '{}'", name);

    let js_wrapper = format!(
        r#"(function() {{
            var old = document.getElementById('template-js');
            if (old) old.remove();
            var oldC = document.getElementById('custom-template-js');
            if (oldC) oldC.remove();
            if (typeof wlog === 'function') wlog('[TEMPLATE] eval wrapper running!');
        }})();
        {}
        if (typeof wlog === 'function') wlog('[TEMPLATE] eval script execution finished');
        "#,
        js
    );
    monitor
        .eval(&js_wrapper)
        .map_err(|e| AppError::WebviewError(format!("JS inject failed: {}", e)))?;
    info!("[inject_custom_template] JS injected for '{}'", name);

    Ok(())
}

fn resolve_builtin_template_dir(name: &str, resource_dir: &Path) -> Option<PathBuf> {
    for candidate in builtin_template_candidates(resource_dir) {
        let path = candidate.join(name);
        if path.is_dir() {
            return Some(path);
        }
    }
    None
}

#[tauri::command]
pub fn make_builtin_editable(name: String, app: tauri::AppHandle) -> Result<(), AppError> {
    validate_template_name(&name)?;

    if !is_builtin(&name) {
        return Err(AppError::ValidationError(format!("'{}' is not a built-in template", name)));
    }

    let base_dir = ensure_user_templates_dir()?;
    let target_dir = base_dir.join(&name);
    if target_dir.exists() {
        return Ok(());
    }

    let resource_dir = get_resource_dir(&app);
    let source_dir = resolve_builtin_template_dir(&name, &resource_dir)
        .ok_or_else(|| AppError::TemplateNotFound(format!("Built-in template '{}' not found", name)))?;

    std::fs::create_dir_all(&target_dir)
        .map_err(|e| AppError::TemplateIoError(format!("Failed to create directory: {}", e)))?;

    if let Ok(entries) = std::fs::read_dir(&source_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(filename) = path.file_name() {
                    let dest = target_dir.join(filename);
                    std::fs::copy(&path, &dest)
                        .map_err(|e| AppError::TemplateIoError(format!("Failed to copy {}: {}", path.display(), e)))?;
                }
            }
        }
    }

    let manifest_path = target_dir.join("manifest.json");
    if !manifest_path.is_file() {
        let manifest = generate_default_manifest(&name, &name);
        std::fs::write(&manifest_path, &manifest)
            .map_err(|e| AppError::TemplateIoError(format!("Failed to write manifest: {}", e)))?;
    }

    info!("Built-in template '{}' copied to user dir for editing", name);
    Ok(())
}

#[tauri::command]
pub fn reset_builtin_template(name: String) -> Result<(), AppError> {
    validate_template_name(&name)?;

    if !is_builtin(&name) {
        return Err(AppError::ValidationError(format!("'{}' is not a built-in template", name)));
    }

    let base_dir = ensure_user_templates_dir()?;
    let user_dir = base_dir.join(&name);
    if !user_dir.exists() {
        return Ok(());
    }

    let canonical_dir = user_dir
        .canonicalize()
        .map_err(|e| AppError::InvalidTemplatePath(format!("Failed to resolve template path: {}", e)))?;
    if !canonical_dir.starts_with(&base_dir) {
        return Err(AppError::InvalidTemplatePath("Invalid template path".into()));
    }

    std::fs::remove_dir_all(&canonical_dir).map_err(|e| {
        error!("Failed to reset template '{}': {}", name, e);
        AppError::TemplateIoError(format!("Failed to reset template: {}", e))
    })?;

    info!("Built-in template '{}' reset to default", name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_template_name() {
        assert!(validate_template_name("my-template_01").is_ok());
        assert!(validate_template_name("v1").is_ok());
        assert!(validate_template_name("").is_err());
        assert!(validate_template_name("MyTemplate").is_err()); // uppercase rejected
        assert!(validate_template_name("../traversal").is_err());
        assert!(validate_template_name("foo/bar").is_err());
        assert!(validate_template_name("foo\\bar").is_err());
    }

    #[test]
    fn test_is_builtin() {
        assert!(is_builtin("v1"));
        assert!(is_builtin("v2"));
        assert!(!is_builtin("custom"));
    }

    #[test]
    fn test_format_display_name() {
        assert_eq!(format_display_name("cpu-temp_monitor"), "Cpu Temp Monitor");
        assert_eq!(format_display_name("v1"), "V1");
    }

    #[test]
    fn test_generate_default_manifest() {
        let manifest_str = generate_default_manifest("v2", "my-cloned");
        let parsed: serde_json::Value = serde_json::from_str(&manifest_str).expect("Valid JSON");
        assert_eq!(parsed["name"], "my-cloned");
        assert_eq!(parsed["canvasWidth"], 480);
        assert_eq!(parsed["canvasHeight"], 320);
        let widgets = parsed["widgets"].as_array();
        assert!(widgets.is_some());
        assert!(widgets.map_or(0, |w| w.len()) >= 6);
    }
}
