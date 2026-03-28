// Widget Registry — defines all available widget types for the template editor.
// Each type has default dimensions, config schema, style schema, and an icon.

var SENSOR_FIELDS = [
  { value: 'cpu_usage', label: 'CPU Usage (%)' },
  { value: 'cpu_temp', label: 'CPU Temperature (C)' },
  { value: 'cpu_freq', label: 'CPU Frequency (MHz)' },
  { value: 'gpu_usage', label: 'GPU Usage (%)' },
  { value: 'gpu_temp', label: 'GPU Temperature (C)' },
  { value: 'gpu_mem_used', label: 'GPU Memory Used (bytes)' },
  { value: 'gpu_mem_total', label: 'GPU Memory Total (bytes)' },
  { value: 'gpu_freq', label: 'GPU Frequency (MHz)' },
  { value: 'ram_used', label: 'RAM Used (bytes)' },
  { value: 'ram_total', label: 'RAM Total (bytes)' },
  { value: 'ram_usage', label: 'RAM Usage (%)' },
  { value: 'disk_used', label: 'Disk Used (bytes)' },
  { value: 'disk_total', label: 'Disk Total (bytes)' },
  { value: 'disk_usage', label: 'Disk Usage (%)' },
  { value: 'net_upload', label: 'Network Upload (bytes/s)' },
  { value: 'net_download', label: 'Network Download (bytes/s)' }
];

var ICON_OPTIONS = [
  { value: 'cpu', label: 'CPU Chip' },
  { value: 'gpu', label: 'GPU Monitor' },
  { value: 'memory', label: 'Memory Stick' },
  { value: 'disk', label: 'Disk Database' },
  { value: 'network', label: 'Network Globe' },
  { value: 'clock', label: 'Clock' },
  { value: 'none', label: 'No Icon' }
];

var WIDGET_ICONS = {
  cpu: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2"/><rect x="9" y="9" width="6" height="6"/><line x1="9" y1="1" x2="9" y2="4"/><line x1="15" y1="1" x2="15" y2="4"/><line x1="9" y1="20" x2="9" y2="23"/><line x1="15" y1="20" x2="15" y2="23"/><line x1="20" y1="9" x2="23" y2="9"/><line x1="20" y1="14" x2="23" y2="14"/><line x1="1" y1="9" x2="4" y2="9"/><line x1="1" y1="14" x2="4" y2="14"/></svg>',
  gpu: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="7" width="20" height="14" rx="2"/><path d="M6 7V5a2 2 0 0 1 2-2h8a2 2 0 0 1 2 2v2"/><line x1="6" y1="12" x2="6" y2="12"/><line x1="10" y1="12" x2="10" y2="12"/><line x1="14" y1="12" x2="14" y2="12"/><line x1="18" y1="12" x2="18" y2="12"/></svg>',
  memory: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="20" height="12" rx="2"/><line x1="6" y1="10" x2="6" y2="14"/><line x1="10" y1="10" x2="10" y2="14"/><line x1="14" y1="10" x2="14" y2="14"/><line x1="18" y1="10" x2="18" y2="14"/></svg>',
  disk: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>',
  network: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>',
  clock: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>',
  none: ''
};

var WIDGET_TYPES = {
  'metric-card': {
    label: 'Metric Card',
    description: 'Full card with header, value, progress bar, and sparkline',
    defaultWidth: 155,
    defaultHeight: 155,
    minWidth: 100,
    minHeight: 80,
    config: {
      title: { type: 'text', default: 'METRIC', label: 'Title' },
      theme: { type: 'select', default: 'v2', label: 'Theme (v1/v2)', options: [ {value: 'v1', label: 'v1 - Simple'}, {value: 'v2', label: 'v2 - Modern'} ] },
      icon: { type: 'select', default: 'cpu', label: 'Icon', options: ICON_OPTIONS },
      primaryField: { type: 'sensor', default: 'cpu_temp', label: 'Primary Value' },
      primaryUnit: { type: 'text', default: '\u00b0C', label: 'Primary Unit' },
      secondaryFields: {
        type: 'sensor-list',
        default: [
          { field: 'cpu_freq', unit: 'MHz' },
          { field: 'cpu_usage', unit: '%' }
        ],
        label: 'Secondary Values'
      },
      progressField: { type: 'sensor', default: 'cpu_usage', label: 'Progress Bar Field' },
      sparklineField: { type: 'sensor', default: 'cpu_usage', label: 'Sparkline Field' },
      showSparkline: { type: 'boolean', default: true, label: 'Show Sparkline' },
      showProgress: { type: 'boolean', default: true, label: 'Show Progress Bar' },
      thresholds: {
        type: 'thresholds',
        default: { warning: 60, critical: 80 },
        label: 'Thresholds'
      }
    }
  },

  'value-display': {
    label: 'Value Display',
    description: 'Single large value with unit label',
    defaultWidth: 120,
    defaultHeight: 60,
    minWidth: 60,
    minHeight: 30,
    config: {
      field: { type: 'sensor', default: 'cpu_temp', label: 'Data Field' },
      unit: { type: 'text', default: '\u00b0C', label: 'Unit' },
      label: { type: 'text', default: '', label: 'Label (optional)' },
      formatAsBytes: { type: 'boolean', default: false, label: 'Format as Bytes' }
    }
  },

  'progress-bar': {
    label: 'Progress Bar',
    description: 'Standalone horizontal progress bar',
    defaultWidth: 200,
    defaultHeight: 30,
    minWidth: 80,
    minHeight: 20,
    config: {
      field: { type: 'sensor', default: 'cpu_usage', label: 'Data Field' },
      showLabel: { type: 'boolean', default: true, label: 'Show Percentage Label' },
      thresholds: {
        type: 'thresholds',
        default: { warning: 70, critical: 90 },
        label: 'Thresholds'
      }
    }
  },

  'sparkline': {
    label: 'Sparkline Chart',
    description: 'Standalone time-series chart',
    defaultWidth: 200,
    defaultHeight: 60,
    minWidth: 80,
    minHeight: 30,
    config: {
      field: { type: 'sensor', default: 'cpu_usage', label: 'Data Field' },
      maxPoints: { type: 'number', default: 120, label: 'Max Data Points', min: 20, max: 500 }
    }
  },

  'clock': {
    label: 'Clock',
    description: 'Time and date display',
    defaultWidth: 155,
    defaultHeight: 155,
    minWidth: 80,
    minHeight: 50,
    config: {
      theme: { type: 'select', default: 'v2', label: 'Theme (v1/v2)', options: [ {value: 'v1', label: 'v1 - Simple'}, {value: 'v2', label: 'v2 - Modern'} ] },
      format24h: { type: 'boolean', default: true, label: '24-Hour Format' },
      showDate: { type: 'boolean', default: true, label: 'Show Date' },
      showSeconds: { type: 'boolean', default: true, label: 'Show Seconds' }
    }
  },

  'network-pair': {
    label: 'Network Upload/Download',
    description: 'Upload and download speed pair',
    defaultWidth: 155,
    defaultHeight: 155,
    minWidth: 100,
    minHeight: 60,
    config: {
      theme: { type: 'select', default: 'v2', label: 'Theme (v1/v2)', options: [ {value: 'v1', label: 'v1 - Simple'}, {value: 'v2', label: 'v2 - Modern'} ] },
      showSparkline: { type: 'boolean', default: true, label: 'Show Sparkline' },
      maxPoints: { type: 'number', default: 120, label: 'Max Data Points', min: 20, max: 500 }
    }
  },

  'label': {
    label: 'Text Label',
    description: 'Static text label',
    defaultWidth: 100,
    defaultHeight: 30,
    minWidth: 30,
    minHeight: 15,
    config: {
      text: { type: 'text', default: 'Label', label: 'Text Content' },
      align: { type: 'select', default: 'left', label: 'Alignment', options: [
        { value: 'left', label: 'Left' },
        { value: 'center', label: 'Center' },
        { value: 'right', label: 'Right' }
      ]}
    }
  },

  'divider': {
    label: 'Divider',
    description: 'Horizontal or vertical line',
    defaultWidth: 200,
    defaultHeight: 4,
    minWidth: 10,
    minHeight: 2,
    config: {
      orientation: { type: 'select', default: 'horizontal', label: 'Orientation', options: [
        { value: 'horizontal', label: 'Horizontal' },
        { value: 'vertical', label: 'Vertical' }
      ]}
    }
  }
};

// Default style properties shared across all widget types
var DEFAULT_STYLE = {
  backgroundColor: '#16161e',
  borderColor: 'rgba(255,255,255,0.06)',
  borderRadius: 8,
  fontFamily: "'Inter', 'Segoe UI', system-ui, sans-serif",
  primaryFontSize: 20,
  secondaryFontSize: 11,
  headerFontSize: 8.5,
  textColor: '#e8ecf1',
  secondaryTextColor: '#6b7280',
  normalColor: '#22c55e',
  warningColor: '#f59e0b',
  criticalColor: '#ef4444'
};

/**
 * Create a new widget with default values for the given type.
 * @param {string} type - Widget type key from WIDGET_TYPES
 * @param {number} x - X position
 * @param {number} y - Y position
 * @returns {object} Widget object
 */
function createWidget(type, x, y) {
  var widgetType = WIDGET_TYPES[type];
  if (!widgetType) return null;

  var config = {};
  var keys = Object.keys(widgetType.config);
  for (var i = 0; i < keys.length; i++) {
    var key = keys[i];
    var def = widgetType.config[key].default;
    // Deep copy arrays and objects
    if (Array.isArray(def)) {
      config[key] = JSON.parse(JSON.stringify(def));
    } else if (def && typeof def === 'object') {
      config[key] = JSON.parse(JSON.stringify(def));
    } else {
      config[key] = def;
    }
  }

  return {
    id: 'w-' + Date.now() + '-' + Math.random().toString(36).substr(2, 6),
    type: type,
    x: x || 0,
    y: y || 0,
    width: widgetType.defaultWidth,
    height: widgetType.defaultHeight,
    config: config,
    style: JSON.parse(JSON.stringify(DEFAULT_STYLE))
  };
}
