// Manifest module — create, validate, and manipulate template manifests.
// All operations return new objects (immutable).

/**
 * Create an empty manifest with default values.
 * @param {string} name - Template name (slug)
 * @returns {object} New manifest
 */
function createEmptyManifest(name) {
  return {
    version: 1,
    name: name || 'untitled',
    displayName: name ? formatDisplayName(name) : 'Untitled Template',
    author: '',
    description: '',
    canvasWidth: 480,
    canvasHeight: 320,
    backgroundColor: '#0c0c10',
    widgets: []
  };
}

/**
 * Format a slug into a display name.
 * @param {string} name
 * @returns {string}
 */
function formatDisplayName(name) {
  return name.split(/[-_]/).map(function(w) {
    return w.charAt(0).toUpperCase() + w.slice(1);
  }).join(' ');
}

/**
 * Validate a manifest structure. Returns an array of error strings (empty = valid).
 * @param {object} manifest
 * @returns {string[]}
 */
function validateManifest(manifest) {
  var errors = [];

  if (!manifest) {
    return ['Manifest is null or undefined'];
  }
  if (manifest.version !== 1) {
    errors.push('Unsupported manifest version: ' + manifest.version);
  }
  if (!manifest.name || typeof manifest.name !== 'string') {
    errors.push('Missing or invalid template name');
  } else if (!/^[a-z0-9_-]{1,64}$/.test(manifest.name)) {
    errors.push('Template name must be 1-64 lowercase chars, digits, hyphens, underscores');
  }
  if (manifest.canvasWidth !== 480 || manifest.canvasHeight !== 320) {
    errors.push('Canvas must be 480x320');
  }
  if (!Array.isArray(manifest.widgets)) {
    errors.push('Widgets must be an array');
  } else {
    var ids = {};
    for (var i = 0; i < manifest.widgets.length; i++) {
      var w = manifest.widgets[i];
      if (!w.id) {
        errors.push('Widget at index ' + i + ' has no id');
      } else if (ids[w.id]) {
        errors.push('Duplicate widget id: ' + w.id);
      } else {
        ids[w.id] = true;
      }
      if (!w.type || !WIDGET_TYPES[w.type]) {
        errors.push('Widget ' + (w.id || i) + ' has invalid type: ' + w.type);
      }
      if (typeof w.x !== 'number' || typeof w.y !== 'number') {
        errors.push('Widget ' + (w.id || i) + ' has invalid position');
      }
      if (typeof w.width !== 'number' || typeof w.height !== 'number') {
        errors.push('Widget ' + (w.id || i) + ' has invalid dimensions');
      }
    }
  }

  return errors;
}

/**
 * Add a widget to a manifest. Returns a new manifest.
 * @param {object} manifest
 * @param {object} widget
 * @returns {object}
 */
function addWidgetToManifest(manifest, widget) {
  return Object.assign({}, manifest, {
    widgets: manifest.widgets.concat([widget])
  });
}

/**
 * Remove a widget from a manifest by id. Returns a new manifest.
 * @param {object} manifest
 * @param {string} widgetId
 * @returns {object}
 */
function removeWidgetFromManifest(manifest, widgetId) {
  return Object.assign({}, manifest, {
    widgets: manifest.widgets.filter(function(w) { return w.id !== widgetId; })
  });
}

/**
 * Update a widget in a manifest. Returns a new manifest with the widget replaced.
 * @param {object} manifest
 * @param {string} widgetId
 * @param {object} changes - Partial widget object to merge
 * @returns {object}
 */
function updateWidgetInManifest(manifest, widgetId, changes) {
  return Object.assign({}, manifest, {
    widgets: manifest.widgets.map(function(w) {
      if (w.id !== widgetId) return w;
      var updated = Object.assign({}, w);
      // Merge top-level keys
      var keys = Object.keys(changes);
      for (var i = 0; i < keys.length; i++) {
        var key = keys[i];
        if (key === 'config') {
          updated.config = Object.assign({}, w.config, changes.config);
        } else if (key === 'style') {
          updated.style = Object.assign({}, w.style, changes.style);
        } else {
          updated[key] = changes[key];
        }
      }
      return updated;
    })
  });
}

/**
 * Update manifest-level properties (not widgets). Returns a new manifest.
 * @param {object} manifest
 * @param {object} changes
 * @returns {object}
 */
function updateManifestProperties(manifest, changes) {
  var result = Object.assign({}, manifest);
  var keys = Object.keys(changes);
  for (var i = 0; i < keys.length; i++) {
    if (keys[i] !== 'widgets') {
      result[keys[i]] = changes[keys[i]];
    }
  }
  return result;
}

/**
 * Serialize manifest to a formatted JSON string.
 * @param {object} manifest
 * @returns {string}
 */
function serializeManifest(manifest) {
  return JSON.stringify(manifest, null, 2);
}

/**
 * Parse a JSON string into a manifest.
 * @param {string} jsonString
 * @returns {object}
 */
function parseManifest(jsonString) {
  return JSON.parse(jsonString);
}
