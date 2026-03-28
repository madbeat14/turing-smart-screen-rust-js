// Editor Application — main controller that wires everything together.

(function() {
  var invoke = window.__TAURI__.core.invoke;
  var manifest = null;
  var canvas = null;
  var isDirty = false;
  var undoStack = [];
  var redoStack = [];
  var MAX_UNDO = 50;

  // DOM references
  var $templateList = document.getElementById('template-list');
  var $canvasContainer = document.getElementById('canvas-container');
  var $propsPanel = document.getElementById('props-panel');
  var $templateName = document.getElementById('template-name');
  var $displayName = document.getElementById('display-name');
  var $bgColor = document.getElementById('bg-color');
  var $status = document.getElementById('editor-status');
  var $previewFrame = document.getElementById('preview-frame');

  // ── Initialize ──────────────────────────────────────────────

  function init() {
    canvas = createCanvas($canvasContainer, {
      onSelect: onWidgetSelect,
      onMove: onWidgetMove,
      onResize: onWidgetResize,
      onDelete: onWidgetDelete,
      onDrop: onWidgetDrop
    });

    loadTemplateList();
    newTemplate();
    bindToolbar();
    bindPalette();
    bindManifestFields();
  }

  // ── Template List ───────────────────────────────────────────

  function loadTemplateList() {
    invoke('list_templates').then(function(templates) {
      $templateList.innerHTML = '';

      for (var i = 0; i < templates.length; i++) {
        var t = templates[i];
        var item = document.createElement('div');
        item.className = 'template-list-item';
        item.dataset.name = t.name;

        var nameSpan = document.createElement('span');
        nameSpan.className = 'tli-name';
        nameSpan.textContent = t.display_name;
        item.appendChild(nameSpan);

        if (t.is_builtin) {
          var badge = document.createElement('span');
          badge.className = 'tli-badge';
          badge.textContent = 'built-in';
          item.appendChild(badge);
        }

        if (t.has_manifest) {
          var editBtn = document.createElement('button');
          editBtn.className = 'tli-btn';
          editBtn.textContent = 'Edit';
          editBtn.dataset.name = t.name;
          editBtn.addEventListener('click', function(e) {
            e.stopPropagation();
            loadTemplate(e.target.dataset.name);
          });
          item.appendChild(editBtn);
        }

        var cloneBtn = document.createElement('button');
        cloneBtn.className = 'tli-btn';
        cloneBtn.textContent = 'Clone';
        cloneBtn.dataset.name = t.name;
        cloneBtn.addEventListener('click', function(e) {
          e.stopPropagation();
          cloneTemplate(e.target.dataset.name);
        });
        item.appendChild(cloneBtn);

        if (!t.is_builtin) {
          var delBtn = document.createElement('button');
          delBtn.className = 'tli-btn tli-btn-danger';
          delBtn.textContent = 'Del';
          delBtn.dataset.name = t.name;
          delBtn.addEventListener('click', function(e) {
            e.stopPropagation();
            deleteTemplate(e.target.dataset.name);
          });
          item.appendChild(delBtn);
        }

        $templateList.appendChild(item);
      }
    }).catch(function(e) {
      showStatus('Failed to load templates: ' + e, true);
    });
  }

  function loadTemplate(name) {
    if (isDirty && !confirm('You have unsaved changes. Discard them?')) return;

    invoke('read_template_manifest', { name: name }).then(function(json) {
      manifest = parseManifest(json);
      $templateName.value = manifest.name;
      $displayName.value = manifest.displayName || '';
      $bgColor.value = manifest.backgroundColor || '#0c0c10';
      isDirty = false;
      undoStack = [];
      redoStack = [];
      canvas.render(manifest.widgets);
      renderPropertiesPanel($propsPanel, null, onWidgetChange);
      showStatus('Loaded: ' + name, false);
    }).catch(function(e) {
      showStatus('Failed to load: ' + e, true);
    });
  }

  function cloneTemplate(sourceName) {
    var targetName = prompt('New template name (lowercase, hyphens, underscores):', sourceName + '-copy');
    if (!targetName) return;
    targetName = targetName.trim().toLowerCase();

    if (!/^[a-z0-9_-]{1,64}$/.test(targetName)) {
      showStatus('Invalid name. Use lowercase letters, digits, hyphens, underscores.', true);
      return;
    }

    invoke('clone_template', { source: sourceName, target: targetName }).then(function() {
      showStatus('Cloned "' + sourceName + '" as "' + targetName + '"', false);
      loadTemplateList();
      loadTemplate(targetName);
    }).catch(function(e) {
      showStatus('Clone failed: ' + e, true);
    });
  }

  function deleteTemplate(name) {
    if (!confirm('Delete template "' + name + '"? This cannot be undone.')) return;

    invoke('delete_template', { name: name }).then(function() {
      showStatus('Deleted: ' + name, false);
      loadTemplateList();
      if (manifest && manifest.name === name) {
        newTemplate();
      }
    }).catch(function(e) {
      showStatus('Delete failed: ' + e, true);
    });
  }

  // ── New Template ────────────────────────────────────────────

  function newTemplate() {
    if (isDirty && !confirm('You have unsaved changes. Discard them?')) return;

    manifest = createEmptyManifest('my-template');
    $templateName.value = manifest.name;
    $displayName.value = manifest.displayName;
    $bgColor.value = manifest.backgroundColor;
    isDirty = false;
    undoStack = [];
    redoStack = [];
    canvas.render(manifest.widgets);
    renderPropertiesPanel($propsPanel, null, onWidgetChange);
  }

  // ── Save ────────────────────────────────────────────────────

  function saveTemplate() {
    if (!manifest) return;

    var name = $templateName.value.trim().toLowerCase();
    if (!/^[a-z0-9_-]{1,64}$/.test(name)) {
      showStatus('Invalid template name.', true);
      return;
    }

    manifest = updateManifestProperties(manifest, {
      name: name,
      displayName: $displayName.value.trim() || formatDisplayName(name)
    });

    var errors = validateManifest(manifest);
    if (errors.length > 0) {
      showStatus('Validation errors: ' + errors.join('; '), true);
      return;
    }

    var html = compileHtml(manifest);
    var css = compileCss(manifest);
    var js = compileJs(manifest);
    var manifestJson = serializeManifest(manifest);

    invoke('save_template', {
      args: {
        name: name,
        manifest: manifestJson,
        html: html,
        css: css,
        js: js
      }
    }).then(function() {
      isDirty = false;
      showStatus('Saved: ' + name, false);
      loadTemplateList();
    }).catch(function(e) {
      showStatus('Save failed: ' + e, true);
    });
  }

  // ── Apply to Monitor ───────────────────────────────────────

  function applyToMonitor() {
    if (!manifest) return;

    var name = $templateName.value.trim().toLowerCase();
    if (!/^[a-z0-9_-]{1,64}$/.test(name)) {
      showStatus('Invalid template name.', true);
      return;
    }

    manifest = updateManifestProperties(manifest, {
      name: name,
      displayName: $displayName.value.trim() || formatDisplayName(name)
    });

    var errors = validateManifest(manifest);
    if (errors.length > 0) {
      showStatus('Validation errors: ' + errors.join('; '), true);
      return;
    }

    var html = compileHtml(manifest);
    var css = compileCss(manifest);
    var js = compileJs(manifest);
    var manifestJson = serializeManifest(manifest);

    showStatus('Saving and applying to monitor...', false);

    // Step 1: Save the template files
    invoke('save_template', {
      args: {
        name: name,
        manifest: manifestJson,
        html: html,
        css: css,
        js: js
      }
    }).then(function() {
      isDirty = false;
      loadTemplateList();

      // Step 2: Get current config and update the THEME
      return invoke('get_config');
    }).then(function(cfg) {
      var newConfig = {
        config: {
          COM_PORT: cfg.config.COM_PORT,
          THEME: name,
          HW_SENSORS: cfg.config.HW_SENSORS,
          ETH: cfg.config.ETH,
          WLO: cfg.config.WLO,
          CPU_FAN: cfg.config.CPU_FAN,
          PING: cfg.config.PING,
          WEATHER_API_KEY: '',
          WEATHER_LATITUDE: cfg.config.WEATHER_LATITUDE,
          WEATHER_LONGITUDE: cfg.config.WEATHER_LONGITUDE,
          WEATHER_UNITS: cfg.config.WEATHER_UNITS,
          WEATHER_LANGUAGE: cfg.config.WEATHER_LANGUAGE
        },
        display: {
          REVISION: cfg.display.REVISION,
          BRIGHTNESS: cfg.display.BRIGHTNESS,
          DISPLAY_REVERSE: cfg.display.DISPLAY_REVERSE,
          RESET_ON_STARTUP: cfg.display.RESET_ON_STARTUP
        }
      };
      return invoke('save_config', { newConfig: newConfig });
    }).then(function() {
      // Step 3: Reload the monitor webview and restart display
      return invoke('reload_monitor');
    }).then(function() {
      return invoke('restart_display');
    }).then(function() {
      showStatus('Applied "' + name + '" to monitor!', false);
    }).catch(function(e) {
      showStatus('Apply failed: ' + e, true);
    });
  }

  // ── Preview ─────────────────────────────────────────────────

  function showPreview() {
    if (!manifest) return;

    var html = compileHtml(manifest);
    var css = compileCss(manifest);
    var js = compileJs(manifest);

    var previewHtml = '<!DOCTYPE html><html><head><meta charset="UTF-8">' +
      '<style>' + css + '</style></head><body>' + html +
      '<script>' + getMockDataScript() + '</' + 'script>' +
      '<script>' + js + '</' + 'script></body></html>';

    $previewFrame.srcdoc = previewHtml;
    $previewFrame.style.display = 'block';
    document.getElementById('canvas-container').style.display = 'none';
    document.getElementById('btn-preview').textContent = 'Back to Editor';
    document.getElementById('btn-preview').dataset.previewing = 'true';
  }

  function hidePreview() {
    $previewFrame.style.display = 'none';
    document.getElementById('canvas-container').style.display = 'flex';
    document.getElementById('btn-preview').textContent = 'Preview';
    document.getElementById('btn-preview').dataset.previewing = '';
  }

  function getMockDataScript() {
    return [
      'var mockData = {',
      '  cpu_usage: 45, cpu_temp: 62, cpu_freq: 3800,',
      '  gpu_usage: 30, gpu_temp: 55, gpu_mem_used: 4294967296, gpu_mem_total: 8589934592, gpu_freq: 1950,',
      '  ram_used: 13312442368, ram_total: 34359738368,',
      '  disk_used: 274877906944, disk_total: 549755813888,',
      '  net_upload: 12800, net_download: 46080',
      '};',
      'setInterval(function() {',
      '  mockData.cpu_usage = 30 + Math.random() * 40;',
      '  mockData.cpu_temp = 55 + Math.random() * 20;',
      '  mockData.gpu_usage = 20 + Math.random() * 30;',
      '  mockData.gpu_temp = 50 + Math.random() * 15;',
      '  mockData.net_upload = Math.random() * 102400;',
      '  mockData.net_download = Math.random() * 512000;',
      '  if (typeof updateUI === "function") updateUI(mockData);',
      '}, 500);'
    ].join('\n');
  }

  // ── Undo/Redo ───────────────────────────────────────────────

  function pushUndo() {
    undoStack.push(JSON.stringify(manifest));
    if (undoStack.length > MAX_UNDO) undoStack.shift();
    redoStack = [];
  }

  function undo() {
    if (undoStack.length === 0) return;
    redoStack.push(JSON.stringify(manifest));
    manifest = JSON.parse(undoStack.pop());
    syncUIFromManifest();
  }

  function redo() {
    if (redoStack.length === 0) return;
    undoStack.push(JSON.stringify(manifest));
    manifest = JSON.parse(redoStack.pop());
    syncUIFromManifest();
  }

  function syncUIFromManifest() {
    $templateName.value = manifest.name;
    $displayName.value = manifest.displayName || '';
    $bgColor.value = manifest.backgroundColor || '#0c0c10';
    canvas.render(manifest.widgets);
    var selId = canvas.getSelectedId();
    var selWidget = selId ? manifest.widgets.find(function(w) { return w.id === selId; }) : null;
    renderPropertiesPanel($propsPanel, selWidget, onWidgetChange);
  }

  // ── Widget Event Handlers ───────────────────────────────────

  function onWidgetSelect(widgetId) {
    var widget = widgetId ? manifest.widgets.find(function(w) { return w.id === widgetId; }) : null;
    renderPropertiesPanel($propsPanel, widget, onWidgetChange);
  }

  function onWidgetMove(widgetId, x, y) {
    pushUndo();
    manifest = updateWidgetInManifest(manifest, widgetId, { x: x, y: y });
    isDirty = true;
    canvas.render(manifest.widgets);
    // Update position fields if this widget is selected
    var xEl = document.getElementById('pos-x');
    var yEl = document.getElementById('pos-y');
    if (xEl) xEl.value = x;
    if (yEl) yEl.value = y;
  }

  function onWidgetResize(widgetId, x, y, w, h) {
    pushUndo();
    manifest = updateWidgetInManifest(manifest, widgetId, { x: x, y: y, width: w, height: h });
    isDirty = true;
    canvas.render(manifest.widgets);
    var xEl = document.getElementById('pos-x');
    var yEl = document.getElementById('pos-y');
    var wEl = document.getElementById('pos-w');
    var hEl = document.getElementById('pos-h');
    if (xEl) xEl.value = x;
    if (yEl) yEl.value = y;
    if (wEl) wEl.value = w;
    if (hEl) hEl.value = h;
  }

  function onWidgetDelete(widgetId) {
    pushUndo();
    manifest = removeWidgetFromManifest(manifest, widgetId);
    isDirty = true;
    canvas.render(manifest.widgets);
    renderPropertiesPanel($propsPanel, null, onWidgetChange);
  }

  function onWidgetDrop(widgetType, x, y) {
    pushUndo();
    var widget = createWidget(widgetType, x, y);
    if (!widget) return;

    // Ensure widget stays within canvas
    if (widget.x + widget.width > 480) widget.x = 480 - widget.width;
    if (widget.y + widget.height > 320) widget.y = 320 - widget.height;
    if (widget.x < 0) widget.x = 0;
    if (widget.y < 0) widget.y = 0;

    manifest = addWidgetToManifest(manifest, widget);
    isDirty = true;
    canvas.render(manifest.widgets);
    canvas.setSelected(widget.id);
    renderPropertiesPanel($propsPanel, widget, onWidgetChange);
  }

  function onWidgetChange(widgetId, changes) {
    pushUndo();
    manifest = updateWidgetInManifest(manifest, widgetId, changes);
    isDirty = true;
    canvas.render(manifest.widgets);
    // Re-render properties to reflect changes (e.g., sensor list add/remove)
    var widget = manifest.widgets.find(function(w) { return w.id === widgetId; });
    if (widget) renderPropertiesPanel($propsPanel, widget, onWidgetChange);
  }

  // ── Import/Export ───────────────────────────────────────────

  function exportManifest() {
    if (!manifest) return;
    var json = serializeManifest(manifest);
    var blob = new Blob([json], { type: 'application/json' });
    var url = URL.createObjectURL(blob);
    var a = document.createElement('a');
    a.href = url;
    a.download = (manifest.name || 'template') + '-manifest.json';
    a.click();
    URL.revokeObjectURL(url);
    showStatus('Exported manifest', false);
  }

  function importManifest() {
    var input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json';
    input.addEventListener('change', function() {
      if (!input.files || !input.files[0]) return;
      var reader = new FileReader();
      reader.onload = function(e) {
        try {
          var imported = parseManifest(e.target.result);
          var errors = validateManifest(imported);
          if (errors.length > 0) {
            showStatus('Invalid manifest: ' + errors[0], true);
            return;
          }
          pushUndo();
          manifest = imported;
          isDirty = true;
          syncUIFromManifest();
          showStatus('Imported: ' + manifest.name, false);
        } catch (err) {
          showStatus('Failed to parse manifest: ' + err.message, true);
        }
      };
      reader.readAsText(input.files[0]);
    });
    input.click();
  }

  // ── Toolbar Bindings ────────────────────────────────────────

  function bindToolbar() {
    document.getElementById('btn-new').addEventListener('click', newTemplate);
    document.getElementById('btn-save').addEventListener('click', saveTemplate);

    document.getElementById('btn-preview').addEventListener('click', function() {
      if (this.dataset.previewing) {
        hidePreview();
      } else {
        showPreview();
      }
    });

    document.getElementById('btn-apply-monitor').addEventListener('click', applyToMonitor);

    document.getElementById('btn-export').addEventListener('click', exportManifest);
    document.getElementById('btn-import').addEventListener('click', importManifest);

    // Keyboard shortcuts
    document.addEventListener('keydown', function(e) {
      if ((e.ctrlKey || e.metaKey) && e.key === 's') {
        e.preventDefault();
        saveTemplate();
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'z' && !e.shiftKey) {
        e.preventDefault();
        undo();
      }
      if ((e.ctrlKey || e.metaKey) && (e.key === 'y' || (e.key === 'z' && e.shiftKey))) {
        e.preventDefault();
        redo();
      }
    });
  }

  function bindPalette() {
    var paletteItems = document.querySelectorAll('.palette-item');
    for (var i = 0; i < paletteItems.length; i++) {
      (function(item) {
        item.setAttribute('draggable', 'true');
        item.addEventListener('dragstart', function(e) {
          e.dataTransfer.setData('text/widget-type', item.dataset.type);
          e.dataTransfer.effectAllowed = 'copy';
        });

        // Also support click-to-add
        item.addEventListener('click', function() {
          if (!manifest) return;
          pushUndo();
          var widget = createWidget(item.dataset.type, 10, 10);
          if (!widget) return;
          manifest = addWidgetToManifest(manifest, widget);
          isDirty = true;
          canvas.render(manifest.widgets);
          canvas.setSelected(widget.id);
          renderPropertiesPanel($propsPanel, widget, onWidgetChange);
        });
      })(paletteItems[i]);
    }
  }

  function bindManifestFields() {
    $templateName.addEventListener('change', function() {
      if (!manifest) return;
      pushUndo();
      manifest = updateManifestProperties(manifest, { name: $templateName.value.trim().toLowerCase() });
      isDirty = true;
    });
    $displayName.addEventListener('change', function() {
      if (!manifest) return;
      pushUndo();
      manifest = updateManifestProperties(manifest, { displayName: $displayName.value.trim() });
      isDirty = true;
    });
    $bgColor.addEventListener('change', function() {
      if (!manifest) return;
      pushUndo();
      manifest = updateManifestProperties(manifest, { backgroundColor: $bgColor.value });
      isDirty = true;
    });
  }

  // ── Status ──────────────────────────────────────────────────

  function showStatus(msg, isError) {
    $status.textContent = msg;
    $status.className = 'editor-status ' + (isError ? 'error' : 'success');
    if (!isError) {
      setTimeout(function() { $status.textContent = ''; }, 3000);
    }
  }

  // ── Warn on close with unsaved changes ──────────────────────

  window.addEventListener('beforeunload', function(e) {
    if (isDirty) {
      e.preventDefault();
      e.returnValue = '';
    }
  });

  // ── Start ───────────────────────────────────────────────────

  if (document.readyState === 'complete' || document.readyState === 'interactive') {
    init();
  } else {
    document.addEventListener('DOMContentLoaded', init);
  }
})();
