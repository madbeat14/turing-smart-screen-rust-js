/// Template management for the editor.
///
/// Provides Tauri commands for listing, reading, saving, deleting,
/// and cloning templates. Custom templates live next to the executable
/// in a `templates/` directory. Built-in templates (v1, v2) are
/// bundled with the app and are read-only.

use log::{error, info};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

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

/// Returns all candidate directories where built-in templates might live.
/// Checks multiple paths to work in both dev mode and production builds.
fn builtin_template_candidates() -> Vec<PathBuf> {
    let exe_dir = crate::config::AppConfig::config_dir();
    let mut candidates = Vec::new();

    // Production: templates bundled next to exe
    candidates.push(exe_dir.join("templates"));

    // Dev mode: exe is in src-tauri/target/{debug|release}/
    // Frontend dist is at project_root/src/templates/
    // Go up from exe dir to find the project root
    if let Some(target_dir) = exe_dir.parent() {
        if let Some(src_tauri_dir) = target_dir.parent() {
            if let Some(project_root) = src_tauri_dir.parent() {
                candidates.push(project_root.join("src").join("templates"));
            }
        }
    }

    candidates
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

/// Resolve the path to a template directory, checking user dir then built-in locations.
/// User templates take priority over bundled ones.
fn resolve_template_dir(name: &str) -> Option<PathBuf> {
    // Check user templates first
    let user_path = user_templates_dir().join(name);
    if user_path.exists() && user_path.is_dir() {
        return Some(user_path);
    }
    // Check all built-in candidate directories
    for candidate in builtin_template_candidates() {
        let path = candidate.join(name);
        if path.exists() && path.is_dir() {
            return Some(path);
        }
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

    // If the clone has no manifest.json, generate a default one so it's editable.
    // This happens when cloning built-in templates (v1, v2) which don't have manifests.
    let manifest_path = target_dir.join("manifest.json");
    if !manifest_path.exists() {
        let default_manifest = generate_default_manifest(&source, &target);
        std::fs::write(&manifest_path, &default_manifest).map_err(|e| {
            format!("Failed to write manifest: {}", e)
        })?;
        info!("Generated default manifest for cloned template '{}'", target);
    }

    info!("Template '{}' cloned to '{}'", source, target);
    Ok(())
}

/// Generate a default manifest.json that matches the standard 6-card layout.
/// This allows cloned built-in templates to be opened in the editor.
fn generate_default_manifest(source_name: &str, target_name: &str) -> String {
    let display_name = format_display_name(target_name);
    let use_sparklines = source_name != "v1";

    let widget_style = serde_json::json!({
        "backgroundColor": "#16161e",
        "borderColor": "rgba(255,255,255,0.06)",
        "borderRadius": 8,
        "fontFamily": "'Inter', 'Segoe UI', system-ui, sans-serif",
        "primaryFontSize": 20,
        "secondaryFontSize": 11,
        "headerFontSize": 8.5,
        "textColor": "#e8ecf1",
        "secondaryTextColor": "#6b7280",
        "normalColor": "#22c55e",
        "warningColor": "#f59e0b",
        "criticalColor": "#ef4444"
    });

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
                    "title": "CPU", "icon": "cpu",
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
                    "title": "GPU", "icon": "gpu",
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
                    "title": "MEMORY", "icon": "memory",
                    "primaryField": "ram_used", "primaryUnit": "%",
                    "secondaryFields": [],
                    "progressField": "ram_used", "sparklineField": "ram_used",
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
                    "title": "DISK", "icon": "disk",
                    "primaryField": "disk_used", "primaryUnit": "%",
                    "secondaryFields": [],
                    "progressField": "disk_used", "sparklineField": "disk_used",
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

/// Read a specific file from a template directory.
/// Used by the monitor webview to load custom template HTML/CSS/JS via IPC
/// (since custom templates are not in the webview's served directory).
#[derive(Serialize)]
pub struct TemplateFiles {
    pub html: String,
    pub css: String,
    pub js: String,
}

#[tauri::command]
pub fn read_template_files(name: String) -> Result<TemplateFiles, String> {
    validate_template_name(&name)?;

    let dir = resolve_template_dir(&name)
        .ok_or_else(|| format!("Template '{}' not found", name))?;

    let read_file = |filename: &str| -> Result<String, String> {
        let path = dir.join(filename);
        std::fs::read_to_string(&path).map_err(|e| {
            error!("Failed to read {}/{}: {}", name, filename, e);
            format!("Failed to read {}: {}", filename, e)
        })
    };

    Ok(TemplateFiles {
        html: read_file("template.html")?,
        css: read_file("style.css")?,
        js: read_file("app.js")?,
    })
}

/// Returns the absolute filesystem paths of a custom template's files.
/// The webview can use convertFileSrc() on these to create asset:// URLs
/// that load as external resources (bypassing CSP inline restrictions).
#[derive(serde::Serialize)]
pub struct TemplatePaths {
    pub css_path: String,
    pub js_path: String,
}

#[tauri::command]
pub fn get_template_paths(name: String) -> Result<TemplatePaths, String> {
    validate_template_name(&name)?;
    let dir = resolve_template_dir(&name)
        .ok_or_else(|| format!("Template '{}' not found", name))?;

    let css_path = dir.join("style.css");
    let js_path = dir.join("app.js");

    if !css_path.exists() {
        return Err(format!("Template '{}' missing style.css", name));
    }
    if !js_path.exists() {
        return Err(format!("Template '{}' missing app.js", name));
    }

    Ok(TemplatePaths {
        css_path: css_path.to_string_lossy().to_string(),
        js_path: js_path.to_string_lossy().to_string(),
    })
}

/// Inject a custom template's CSS and JS directly into the monitor webview
/// using WebviewWindow::eval(). This bypasses CSP entirely because eval()
/// runs at the browser-engine level, not through the content security policy.
#[tauri::command]
pub fn inject_custom_template(
    name: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    validate_template_name(&name)?;
    let dir = resolve_template_dir(&name)
        .ok_or_else(|| format!("Template '{}' not found", name))?;

    let css = std::fs::read_to_string(dir.join("style.css"))
        .map_err(|e| format!("Failed to read style.css: {}", e))?;
    let js = std::fs::read_to_string(dir.join("app.js"))
        .map_err(|e| format!("Failed to read app.js: {}", e))?;

    let monitor = app
        .get_webview_window("monitor")
        .ok_or("Monitor window not found")?;

    // Inject CSS: create a CSSStyleSheet (bypasses CSP style-src restrictions entirely)
    let css_escaped = css
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace("${", "\\${");
    let css_js = format!(
        r#"(function() {{
            try {{
                var sheet = new CSSStyleSheet();
                sheet.replaceSync(`{}`);
                document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
            }} catch(e) {{
                // Fallback for older WebViews if needed:
                var old = document.getElementById('custom-template-css');
                if (old) old.remove();
                var s = document.createElement('style');
                s.id = 'custom-template-css';
                s.textContent = `{}`;
                document.head.appendChild(s);
            }}
        }})();"#,
        css_escaped, css_escaped
    );
    monitor.eval(&css_js).map_err(|e| format!("CSS inject failed: {}", e))?;
    info!("[inject_custom_template] CSS injected for '{}'", name);

    // Inject JS: execute the template's app.js directly via eval
    // DO NOT escape backslashes or backticks here! We are injecting raw code,
    // not putting a string inside a template literal like we did for CSS.
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
    monitor.eval(&js_wrapper).map_err(|e| format!("JS inject failed: {}", e))?;
    info!("[inject_custom_template] JS injected for '{}'", name);

    Ok(())
}
