// Properties Panel — renders a dynamic form for the selected widget's config and style.

/**
 * Render the properties panel for a widget.
 * @param {HTMLElement} containerEl
 * @param {object} widget - The selected widget object
 * @param {function} onChange - Called with (widgetId, changes) when a property changes
 */
function renderPropertiesPanel(containerEl, widget, onChange) {
  if (!widget) {
    containerEl.innerHTML = '<div class="panel-empty">Select a widget to edit its properties</div>';
    return;
  }

  var typeInfo = WIDGET_TYPES[widget.type];
  if (!typeInfo) {
    containerEl.innerHTML = '<div class="panel-empty">Unknown widget type</div>';
    return;
  }

  var html = [];
  html.push('<div class="panel-section">');
  html.push('<h3>' + escHtml(typeInfo.label) + '</h3>');
  html.push('<p class="panel-desc">' + escHtml(typeInfo.description) + '</p>');
  html.push('</div>');

  // Position & Size
  html.push('<div class="panel-section">');
  html.push('<h4>Position & Size</h4>');
  html.push('<div class="prop-grid">');
  html.push(renderNumberInput('X', 'pos-x', widget.x, 0, 480));
  html.push(renderNumberInput('Y', 'pos-y', widget.y, 0, 320));
  html.push(renderNumberInput('Width', 'pos-w', widget.width, typeInfo.minWidth, 480));
  html.push(renderNumberInput('Height', 'pos-h', widget.height, typeInfo.minHeight, 320));
  html.push('</div>');
  html.push('</div>');

  // Config properties
  var configKeys = Object.keys(typeInfo.config);
  if (configKeys.length > 0) {
    html.push('<div class="panel-section">');
    html.push('<h4>Configuration</h4>');
    for (var i = 0; i < configKeys.length; i++) {
      var key = configKeys[i];
      var schema = typeInfo.config[key];
      var value = widget.config[key];
      if (value === undefined) value = schema.default;
      html.push(renderConfigField('cfg-' + key, key, schema, value));
    }
    html.push('</div>');
  }

  // Style properties
  html.push('<div class="panel-section">');
  html.push('<h4>Appearance</h4>');
  html.push(renderColorInput('Background', 'style-backgroundColor', widget.style.backgroundColor));
  html.push(renderColorInput('Text Color', 'style-textColor', widget.style.textColor));
  html.push(renderColorInput('Normal Color', 'style-normalColor', widget.style.normalColor));
  html.push(renderColorInput('Warning Color', 'style-warningColor', widget.style.warningColor));
  html.push(renderColorInput('Critical Color', 'style-criticalColor', widget.style.criticalColor));
  html.push(renderNumberInput('Primary Font Size', 'style-primaryFontSize', widget.style.primaryFontSize, 8, 48));
  html.push(renderNumberInput('Border Radius', 'style-borderRadius', widget.style.borderRadius, 0, 24));
  html.push('</div>');

  containerEl.innerHTML = html.join('');

  // Bind event listeners
  bindPanelEvents(containerEl, widget, onChange);
}

function renderNumberInput(label, id, value, min, max) {
  return '<div class="prop-field">' +
    '<label for="' + id + '">' + escHtml(label) + '</label>' +
    '<input type="number" id="' + id + '" value="' + value + '" min="' + min + '" max="' + max + '" />' +
    '</div>';
}

function renderColorInput(label, id, value) {
  // For rgba values, just show as text input
  var isHex = value && value.charAt(0) === '#';
  return '<div class="prop-field prop-color">' +
    '<label for="' + id + '">' + escHtml(label) + '</label>' +
    '<div class="color-input-wrap">' +
    (isHex ? '<input type="color" id="' + id + '" value="' + escHtml(value) + '" />' :
      '<input type="text" id="' + id + '" value="' + escHtml(value) + '" />') +
    '</div>' +
    '</div>';
}

function renderConfigField(id, key, schema, value) {
  var html = '<div class="prop-field">';
  html += '<label for="' + id + '">' + escHtml(schema.label) + '</label>';

  switch (schema.type) {
    case 'text':
      html += '<input type="text" id="' + id + '" value="' + escHtml(String(value || '')) + '" />';
      break;

    case 'number':
      var min = schema.min !== undefined ? schema.min : 0;
      var max = schema.max !== undefined ? schema.max : 9999;
      html += '<input type="number" id="' + id + '" value="' + value + '" min="' + min + '" max="' + max + '" />';
      break;

    case 'boolean':
      html += '<input type="checkbox" id="' + id + '"' + (value ? ' checked' : '') + ' />';
      break;

    case 'select':
      html += '<select id="' + id + '">';
      for (var i = 0; i < schema.options.length; i++) {
        var opt = schema.options[i];
        html += '<option value="' + escHtml(opt.value) + '"' + (opt.value === value ? ' selected' : '') + '>' + escHtml(opt.label) + '</option>';
      }
      html += '</select>';
      break;

    case 'sensor':
      html += '<select id="' + id + '">';
      for (var si = 0; si < SENSOR_FIELDS.length; si++) {
        var sf = SENSOR_FIELDS[si];
        html += '<option value="' + sf.value + '"' + (sf.value === value ? ' selected' : '') + '>' + escHtml(sf.label) + '</option>';
      }
      html += '</select>';
      break;

    case 'sensor-list':
      html += renderSensorListField(id, key, value);
      break;

    case 'thresholds':
      html += '<div class="threshold-fields">';
      html += '<label>Warning: <input type="number" id="' + id + '-warning" value="' + (value.warning || 60) + '" min="0" max="100" /></label>';
      html += '<label>Critical: <input type="number" id="' + id + '-critical" value="' + (value.critical || 80) + '" min="0" max="100" /></label>';
      html += '</div>';
      break;

    default:
      html += '<input type="text" id="' + id + '" value="' + escHtml(String(value || '')) + '" />';
  }

  html += '</div>';
  return html;
}

function renderSensorListField(id, key, value) {
  var html = '<div class="sensor-list" id="' + id + '">';
  var items = value || [];
  for (var i = 0; i < items.length; i++) {
    html += '<div class="sensor-list-item">';
    html += '<select class="sl-field" data-index="' + i + '">';
    for (var si = 0; si < SENSOR_FIELDS.length; si++) {
      var sf = SENSOR_FIELDS[si];
      html += '<option value="' + sf.value + '"' + (sf.value === items[i].field ? ' selected' : '') + '>' + escHtml(sf.label) + '</option>';
    }
    html += '</select>';
    html += '<input type="text" class="sl-unit" data-index="' + i + '" value="' + escHtml(items[i].unit || '') + '" placeholder="Unit" />';
    html += '<button class="sl-remove" data-index="' + i + '" title="Remove">&times;</button>';
    html += '</div>';
  }
  html += '<button class="sl-add btn-small">+ Add Field</button>';
  html += '</div>';
  return html;
}

function bindPanelEvents(containerEl, widget, onChange) {
  // Position & Size
  ['pos-x', 'pos-y', 'pos-w', 'pos-h'].forEach(function(id) {
    var el = containerEl.querySelector('#' + id);
    if (!el) return;
    el.addEventListener('change', function() {
      var val = parseInt(el.value, 10);
      if (isNaN(val)) return;
      var key = { 'pos-x': 'x', 'pos-y': 'y', 'pos-w': 'width', 'pos-h': 'height' }[id];
      var changes = {};
      changes[key] = val;
      onChange(widget.id, changes);
    });
  });

  // Config fields
  var typeInfo = WIDGET_TYPES[widget.type];
  var configKeys = Object.keys(typeInfo.config);
  for (var i = 0; i < configKeys.length; i++) {
    (function(key) {
      var schema = typeInfo.config[key];
      var elId = 'cfg-' + key;

      if (schema.type === 'thresholds') {
        var warnEl = containerEl.querySelector('#' + elId + '-warning');
        var critEl = containerEl.querySelector('#' + elId + '-critical');
        if (warnEl) warnEl.addEventListener('change', function() {
          var config = {};
          config[key] = { warning: parseInt(warnEl.value, 10), critical: parseInt(critEl.value, 10) };
          onChange(widget.id, { config: config });
        });
        if (critEl) critEl.addEventListener('change', function() {
          var config = {};
          config[key] = { warning: parseInt(warnEl.value, 10), critical: parseInt(critEl.value, 10) };
          onChange(widget.id, { config: config });
        });
        return;
      }

      if (schema.type === 'sensor-list') {
        var listEl = containerEl.querySelector('#' + elId);
        if (!listEl) return;
        // Bind existing items
        listEl.querySelectorAll('.sl-field').forEach(function(sel) {
          sel.addEventListener('change', function() {
            var items = collectSensorList(listEl);
            var config = {};
            config[key] = items;
            onChange(widget.id, { config: config });
          });
        });
        listEl.querySelectorAll('.sl-unit').forEach(function(inp) {
          inp.addEventListener('change', function() {
            var items = collectSensorList(listEl);
            var config = {};
            config[key] = items;
            onChange(widget.id, { config: config });
          });
        });
        listEl.querySelectorAll('.sl-remove').forEach(function(btn) {
          btn.addEventListener('click', function() {
            var idx = parseInt(btn.dataset.index, 10);
            var current = widget.config[key] || [];
            var updated = current.filter(function(_, ii) { return ii !== idx; });
            var config = {};
            config[key] = updated;
            onChange(widget.id, { config: config });
          });
        });
        var addBtn = listEl.querySelector('.sl-add');
        if (addBtn) addBtn.addEventListener('click', function() {
          var current = widget.config[key] || [];
          var updated = current.concat([{ field: 'cpu_usage', unit: '%' }]);
          var config = {};
          config[key] = updated;
          onChange(widget.id, { config: config });
        });
        return;
      }

      var el = containerEl.querySelector('#' + elId);
      if (!el) return;

      var eventType = schema.type === 'boolean' ? 'change' : 'change';
      el.addEventListener(eventType, function() {
        var val;
        if (schema.type === 'boolean') {
          val = el.checked;
        } else if (schema.type === 'number') {
          val = parseInt(el.value, 10);
        } else {
          val = el.value;
        }
        var config = {};
        config[key] = val;
        onChange(widget.id, { config: config });
      });
    })(configKeys[i]);
  }

  // Style fields
  ['style-backgroundColor', 'style-textColor', 'style-normalColor', 'style-warningColor',
   'style-criticalColor', 'style-primaryFontSize', 'style-borderRadius'].forEach(function(id) {
    var el = containerEl.querySelector('#' + id);
    if (!el) return;
    el.addEventListener('change', function() {
      var styleProp = id.replace('style-', '');
      var val = el.type === 'number' ? parseInt(el.value, 10) : el.value;
      var style = {};
      style[styleProp] = val;
      onChange(widget.id, { style: style });
    });
  });
}

function collectSensorList(listEl) {
  var items = [];
  var fields = listEl.querySelectorAll('.sl-field');
  var units = listEl.querySelectorAll('.sl-unit');
  for (var i = 0; i < fields.length; i++) {
    items.push({ field: fields[i].value, unit: units[i] ? units[i].value : '' });
  }
  return items;
}
