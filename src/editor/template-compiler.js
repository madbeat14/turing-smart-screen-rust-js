// Template Compiler — transforms a manifest into template.html, style.css, and app.js.
// Each function returns a string of generated code.

/**
 * Compile the full template.html from a manifest.
 * @param {object} manifest
 * @returns {string}
 */
function compileHtml(manifest) {
  var lines = [];
  lines.push('  <div class="monitor">');

  for (var i = 0; i < manifest.widgets.length; i++) {
    var w = manifest.widgets[i];
    lines.push(compileWidgetHtml(w));
  }

  lines.push('  </div>');
  return lines.join('\n');
}

function compileWidgetHtml(w) {
  var cls = 'widget w-' + w.id;

  switch (w.type) {
    case 'metric-card':
      return compileMetricCardHtml(w, cls);
    case 'value-display':
      return compileValueDisplayHtml(w, cls);
    case 'progress-bar':
      return compileProgressBarHtml(w, cls);
    case 'sparkline':
      return compileSparklineHtml(w, cls);
    case 'clock':
      return compileClockHtml(w, cls);
    case 'network-pair':
      return compileNetworkPairHtml(w, cls);
    case 'label':
      return compileLabelHtml(w, cls);
    case 'divider':
      return compileDividerHtml(w, cls);
    default:
      return '    <!-- Unknown widget type: ' + escHtml(w.type) + ' -->';
  }
}

function compileMetricCardHtml(w, cls) {
  var c = w.config;
  var iconHtml = c.icon && c.icon !== 'none' && WIDGET_ICONS[c.icon] ? WIDGET_ICONS[c.icon] : '';
  var lines = [];

  if (c.theme === 'v1') {
    lines.push('    <div class="' + cls + ' card v1-card" id="card-' + w.id + '">');
    lines.push('      <div class="card-header">' + escHtml(c.title) + '</div>');
    lines.push('      <div class="card-value" id="primary-' + w.id + '">');
    lines.push('        <span class="value">--</span><span style="font-size:12px">' + escHtml(c.primaryUnit) + '</span>');
    lines.push('      </div>');

    if (c.secondaryFields && c.secondaryFields.length > 0) {
      lines.push('      <div class="card-sub" id="sec-0-' + w.id + '">-- ' + escHtml(c.secondaryFields[0].unit) + '</div>');
    }

    if (c.showProgress && c.progressField) {
      lines.push('      <div class="progress-container">');
      lines.push('        <div class="progress-bar" id="bar-' + w.id + '" style="width: 0%"></div>');
      lines.push('      </div>');
      lines.push('      <div class="card-label" id="bar-label-' + w.id + '">0%</div>');
    }
    lines.push('    </div>');
    return lines.join('\n');
  }

  // Default v2 theme
  lines.push('    <div class="' + cls + ' card state-normal" id="card-' + w.id + '">');
  lines.push('      <div class="card-header">' + iconHtml + ' ' + escHtml(c.title) + '</div>');
  lines.push('      <div class="metrics-row">');
  lines.push('        <div class="metric-primary" id="primary-' + w.id + '">');
  lines.push('          <span class="value">--</span><span class="unit">' + escHtml(c.primaryUnit) + '</span>');
  lines.push('        </div>');

  if (c.secondaryFields && c.secondaryFields.length > 0) {
    lines.push('        <div class="metric-secondary">');
    for (var i = 0; i < c.secondaryFields.length; i++) {
      if (i > 0) lines.push('          <span class="sep">|</span>');
      lines.push('          <span class="value" id="sec-' + i + '-' + w.id + '">--</span><span class="unit">' + escHtml(c.secondaryFields[i].unit) + '</span>');
    }
    lines.push('        </div>');
  }

  lines.push('      </div>');

  if (c.showProgress && c.progressField) {
    lines.push('      <div class="progress-row">');
    lines.push('        <div class="progress-track"><div class="progress-fill" id="bar-' + w.id + '"></div></div>');
    lines.push('        <div class="progress-label" id="bar-label-' + w.id + '">0%</div>');
    lines.push('      </div>');
  }

  if (c.showSparkline && c.sparklineField) {
    lines.push('      <div class="sparkline-container"><canvas id="spark-' + w.id + '"></canvas></div>');
  }

  lines.push('    </div>');
  return lines.join('\n');
}

function compileValueDisplayHtml(w, cls) {
  var c = w.config;
  var lines = [];
  lines.push('    <div class="' + cls + ' value-display">');
  if (c.label) {
    lines.push('      <div class="vd-label">' + escHtml(c.label) + '</div>');
  }
  lines.push('      <div class="vd-value" id="val-' + w.id + '">');
  lines.push('        <span class="value">--</span><span class="unit">' + escHtml(c.unit) + '</span>');
  lines.push('      </div>');
  lines.push('    </div>');
  return lines.join('\n');
}

function compileProgressBarHtml(w, cls) {
  var c = w.config;
  var lines = [];
  lines.push('    <div class="' + cls + ' standalone-progress">');
  lines.push('      <div class="progress-track"><div class="progress-fill" id="bar-' + w.id + '"></div></div>');
  if (c.showLabel) {
    lines.push('      <div class="progress-label" id="bar-label-' + w.id + '">0%</div>');
  }
  lines.push('    </div>');
  return lines.join('\n');
}

function compileSparklineHtml(w, cls) {
  var lines = [];
  lines.push('    <div class="' + cls + ' standalone-sparkline">');
  lines.push('      <div class="sparkline-container"><canvas id="spark-' + w.id + '"></canvas></div>');
  lines.push('    </div>');
  return lines.join('\n');
}

function compileClockHtml(w, cls) {
  var lines = [];
  if (w.config.theme === 'v1') {
    lines.push('    <div class="' + cls + ' card v1-card clock-widget">');
    lines.push('      <div class="card-header">TIME</div>');
    lines.push('      <div class="card-value" id="clock-time-' + w.id + '"><span class="value">--:--</span></div>');
    if (w.config.showDate) {
      lines.push('      <div class="card-sub" id="clock-date-' + w.id + '">----</div>');
    }
    lines.push('    </div>');
    return lines.join('\n');
  }

  lines.push('    <div class="' + cls + ' card clock-widget">');
  lines.push('      <div class="card-header">' + (WIDGET_ICONS.clock || '') + ' TIME</div>');
  lines.push('      <div class="clock-time" id="clock-time-' + w.id + '">--:--:--</div>');
  if (w.config.showDate) {
    lines.push('      <div class="clock-date" id="clock-date-' + w.id + '">---</div>');
  }
  lines.push('    </div>');
  return lines.join('\n');
}

function compileNetworkPairHtml(w, cls) {
  var lines = [];
  if (w.config.theme === 'v1') {
    lines.push('    <div class="' + cls + ' card v1-card network-widget">');
    lines.push('      <div class="card-header">NETWORK</div>');
    lines.push('      <div class="card-value" id="net-up-' + w.id + '"><span class="arrow up">&uarr;</span> <span class="value">--</span> <span class="unit">KB/s</span></div>');
    lines.push('      <div class="card-sub" id="net-down-' + w.id + '"><span class="arrow down">&darr;</span> <span class="value">--</span> <span class="unit">KB/s</span></div>');
    lines.push('    </div>');
    return lines.join('\n');
  }

  lines.push('    <div class="' + cls + ' card network-widget">');
  lines.push('      <div class="card-header">' + (WIDGET_ICONS.network || '') + ' NETWORK</div>');
  lines.push('      <div class="net-metrics">');
  lines.push('        <div class="net-metric" id="net-up-' + w.id + '">');
  lines.push('          <span class="arrow up">&uarr;</span>');
  lines.push('          <span class="value">--</span>');
  lines.push('          <span class="unit">KB/s</span>');
  lines.push('        </div>');
  lines.push('        <div class="net-metric" id="net-down-' + w.id + '">');
  lines.push('          <span class="arrow down">&darr;</span>');
  lines.push('          <span class="value">--</span>');
  lines.push('          <span class="unit">KB/s</span>');
  lines.push('        </div>');
  lines.push('      </div>');
  if (w.config.showSparkline) {
    lines.push('      <div class="sparkline-container"><canvas id="spark-' + w.id + '"></canvas></div>');
  }
  lines.push('    </div>');
  return lines.join('\n');
}

function compileLabelHtml(w, cls) {
  return '    <div class="' + cls + ' text-label">' + escHtml(w.config.text || '') + '</div>';
}

function compileDividerHtml(w, cls) {
  return '    <div class="' + cls + ' divider-widget"><div class="divider-inner"></div></div>';
}


// ── CSS Compiler ──────────────────────────────────────────────

/**
 * Compile style.css from a manifest.
 * @param {object} manifest
 * @returns {string}
 */
function compileCss(manifest) {
  // Merge first widget's style with defaults so missing fields don't produce undefined CSS
  var firstStyle = manifest.widgets.length > 0 ? manifest.widgets[0].style : {};
  var s = Object.assign({}, DEFAULT_STYLE, firstStyle || {});

  var css = [];
  css.push('/* Generated by Template Editor */');
  css.push(':root {');
  css.push('  --bg: ' + manifest.backgroundColor + ';');
  css.push('  --card-bg: ' + s.backgroundColor + ';');
  css.push('  --card-border: ' + s.borderColor + ';');
  css.push('  --card-highlight: rgba(255, 255, 255, 0.04);');
  css.push('  --text-primary: ' + s.textColor + ';');
  css.push('  --text-secondary: ' + s.secondaryTextColor + ';');
  css.push('  --text-muted: rgba(255, 255, 255, 0.35);');
  css.push('  --state-normal: ' + s.normalColor + ';');
  css.push('  --state-normal-dim: ' + hexToRgba(s.normalColor, 0.15) + ';');
  css.push('  --state-warning: ' + s.warningColor + ';');
  css.push('  --state-warning-dim: ' + hexToRgba(s.warningColor, 0.15) + ';');
  css.push('  --state-critical: ' + s.criticalColor + ';');
  css.push('  --state-critical-dim: ' + hexToRgba(s.criticalColor, 0.15) + ';');
  css.push('  --font: ' + s.fontFamily + ';');
  css.push('  --radius: ' + s.borderRadius + 'px;');
  css.push('  --transition: 0.4s ease;');
  css.push('}');
  css.push('');

  // Reset and body
  css.push('*, *::before, *::after { margin: 0; padding: 0; box-sizing: border-box; }');
  css.push('body { width: ' + (manifest.canvasWidth || 480) + 'px; height: ' + (manifest.canvasHeight || 320) + 'px; background: var(--bg); font-family: var(--font); color: var(--text-primary); overflow: hidden; user-select: none; -webkit-font-smoothing: antialiased; }');
  css.push('.monitor { display: block; position: relative; width: 100%; height: 100%; padding: 0; margin: 0; background: var(--bg); }');
  css.push('.widget { position: absolute; }');
  css.push('');

  // Card styles
  css.push('.card { background: var(--card-bg); border: 1px solid var(--card-border); border-radius: var(--radius); padding: 7px 9px 6px; display: flex; flex-direction: column; overflow: hidden; box-shadow: 0 2px 8px rgba(0,0,0,0.4), inset 0 1px 0 var(--card-highlight); }');
  css.push('.card.state-normal { border-color: rgba(34,197,94,0.2); }');
  css.push('.card.state-warning { border-color: rgba(245,158,11,0.25); }');
  css.push('.card.state-critical { border-color: rgba(239,68,68,0.3); box-shadow: 0 2px 8px rgba(0,0,0,0.4), inset 0 1px 0 var(--card-highlight), 0 0 12px rgba(239,68,68,0.1); }');
  css.push('');

  // V1 Card overrides
  css.push('.v1-card { justify-content: center; border: 1px solid rgba(255,255,255,0.1); box-shadow: none; padding: 8px 10px; }');
  css.push('.v1-card .card-header { font-size: 10px; font-weight: 600; color: #8b949e; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 4px; }');
  css.push('.v1-card .card-value { font-size: 22px; font-weight: 700; line-height: 1.1; color: #e6edf3; }');
  css.push('.v1-card .card-sub { font-size: 12px; color: #8b949e; margin-top: 2px; }');
  css.push('.v1-card .card-label { font-size: 11px; color: #8b949e; margin-top: 2px; text-align: right; }');
  css.push('.v1-card .progress-container { width: 100%; height: 6px; background: rgba(255,255,255,0.1); border-radius: 3px; margin-top: 6px; overflow: hidden; }');
  css.push('.v1-card .progress-bar { height: 100%; border-radius: 3px; background: linear-gradient(90deg, #3fb950, #f85149); transition: width 0.3s ease; }');
  css.push('');

  // Card header
  css.push('.card-header { display: flex; align-items: center; gap: 4px; font-size: ' + s.headerFontSize + 'px; font-weight: 600; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 1.2px; margin-bottom: 3px; line-height: 1; }');
  css.push('.card-header svg { width: 10px; height: 10px; opacity: 0.45; flex-shrink: 0; }');
  css.push('');

  // Metrics
  css.push('.metrics-row { display: flex; align-items: baseline; justify-content: space-between; gap: 4px; min-height: 20px; }');
  css.push('.metric-primary { display: flex; align-items: baseline; gap: 1px; font-variant-numeric: tabular-nums; transition: color var(--transition); }');
  css.push('.metric-primary .value { font-size: ' + s.primaryFontSize + 'px; font-weight: 700; line-height: 1; }');
  css.push('.metric-primary .unit { font-size: 10px; font-weight: 400; color: var(--text-muted); margin-left: 1px; }');
  css.push('.metric-secondary { display: flex; align-items: baseline; gap: 2px; font-variant-numeric: tabular-nums; color: var(--text-secondary); font-size: ' + s.secondaryFontSize + 'px; white-space: nowrap; }');
  css.push('.metric-secondary .value { font-weight: 600; }');
  css.push('.metric-secondary .unit { font-size: 9px; color: var(--text-muted); }');
  css.push('.metric-secondary .sep { color: rgba(255,255,255,0.12); margin: 0 2px; font-weight: 300; }');
  css.push('.card.state-normal .metric-primary { color: var(--state-normal); }');
  css.push('.card.state-warning .metric-primary { color: var(--state-warning); }');
  css.push('.card.state-critical .metric-primary { color: var(--state-critical); }');
  css.push('');

  // Progress bar
  css.push('.progress-row { display: flex; align-items: center; gap: 5px; margin-top: 3px; }');
  css.push('.progress-track { flex: 1; height: 3px; background: rgba(255,255,255,0.06); border-radius: 2px; overflow: hidden; }');
  css.push('.progress-fill { height: 100%; border-radius: 2px; background: var(--state-normal); transition: width 0.3s ease, background-color var(--transition); box-shadow: 0 0 4px rgba(34,197,94,0.3); }');
  css.push('.card.state-normal .progress-fill { background: var(--state-normal); box-shadow: 0 0 4px rgba(34,197,94,0.3); }');
  css.push('.card.state-warning .progress-fill { background: var(--state-warning); box-shadow: 0 0 4px rgba(245,158,11,0.3); }');
  css.push('.card.state-critical .progress-fill { background: var(--state-critical); box-shadow: 0 0 6px rgba(239,68,68,0.4); }');
  css.push('.progress-label { font-size: 10px; font-weight: 600; font-variant-numeric: tabular-nums; color: var(--text-secondary); min-width: 24px; text-align: right; }');
  css.push('');

  // Sparkline
  css.push('.sparkline-container { flex: 1; min-height: 0; margin-top: 3px; border-radius: 4px; overflow: hidden; background: rgba(255,255,255,0.015); width: 100%; height: 100%; }');
  css.push('.sparkline-container canvas { display: block; width: 100%; height: 100%; }');
  css.push('');

  // Standalone widgets
  css.push('.standalone-progress { display: flex; align-items: center; gap: 5px; padding: 4px; }');
  css.push('.standalone-sparkline { padding: 0; }');
  css.push('');

  // Value display
  css.push('.value-display { display: flex; flex-direction: column; justify-content: center; align-items: center; padding: 4px; }');
  css.push('.vd-label { font-size: ' + s.headerFontSize + 'px; font-weight: 600; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 1px; margin-bottom: 2px; }');
  css.push('.vd-value { display: flex; align-items: baseline; gap: 2px; }');
  css.push('.vd-value .value { font-size: ' + s.primaryFontSize + 'px; font-weight: 700; color: var(--text-primary); }');
  css.push('.vd-value .unit { font-size: 10px; color: var(--text-muted); }');
  css.push('');

  // Network
  css.push('.net-metrics { display: flex; flex-direction: column; gap: 1px; }');
  css.push('.net-metric { display: flex; align-items: baseline; gap: 3px; font-variant-numeric: tabular-nums; font-size: 13px; }');
  css.push('.net-metric .arrow { font-size: 10px; font-weight: 700; }');
  css.push('.net-metric .arrow.up { color: #22c55e; }');
  css.push('.net-metric .arrow.down { color: #3b82f6; }');
  css.push('.net-metric .value { font-weight: 700; }');
  css.push('.net-metric .unit { font-size: 9px; color: var(--text-muted); }');
  css.push('');

  // Clock
  css.push('.clock-time { font-size: 28px; font-weight: 700; font-variant-numeric: tabular-nums; line-height: 1; letter-spacing: 0.5px; }');
  css.push('.clock-date { font-size: 10px; color: var(--text-secondary); margin-top: 4px; }');
  css.push('');

  // Text label
  css.push('.text-label { display: flex; align-items: center; font-size: ' + s.secondaryFontSize + 'px; color: var(--text-primary); padding: 2px 4px; }');
  css.push('');

  // Divider
  css.push('.divider-widget { display: flex; align-items: center; justify-content: center; }');
  css.push('.divider-inner { background: rgba(255,255,255,0.1); border-radius: 1px; }');
  css.push('');

  // Per-widget style and positioning overrides.
  // Moved to CSS classes because inline styles are blocked by Tauri v2 CSP.
  for (var i = 0; i < manifest.widgets.length; i++) {
    var w = manifest.widgets[i];
    
    // Position and Size
    css.push('.w-' + w.id + ' { position: absolute; left: ' + w.x + 'px; top: ' + w.y + 'px; width: ' + w.width + 'px; height: ' + w.height + 'px; }');
    
    // Custom Background Color
    if (w.style && w.style.backgroundColor !== s.backgroundColor) {
      css.push('.w-' + w.id + '.card { background: ' + w.style.backgroundColor + '; }');
    }
    
    // Label Alignment
    if (w.type === 'label' && w.config.align) {
      css.push('.w-' + w.id + '.text-label { justify-content: ' + (w.config.align === 'center' ? 'center' : (w.config.align === 'right' ? 'flex-end' : 'flex-start')) + '; text-align: ' + w.config.align + '; }');
    }
    
    // Divider Orientation
    if (w.type === 'divider') {
      var isVert = w.config.orientation === 'vertical';
      var dW = isVert ? '2px' : '100%';
      var dH = isVert ? '100%' : '2px';
      var dM = isVert ? '0 auto' : 'auto 0';
      css.push('.w-' + w.id + ' .divider-inner { width: ' + dW + '; height: ' + dH + '; margin: ' + dM + '; }');
    }
  }


  return css.join('\n');
}


// ── JS Compiler ───────────────────────────────────────────────

/**
 * Compile app.js from a manifest.
 * @param {object} manifest
 * @returns {string}
 */
function compileJs(manifest) {
  var js = [];

  js.push('// Generated by Template Editor');
  js.push('');

  // Sparkline factory (same as v2)
  js.push(SPARKLINE_FACTORY_CODE);
  js.push('');

  // Net sparkline factory
  js.push(NET_SPARKLINE_FACTORY_CODE);
  js.push('');

  // State helpers
  js.push(STATE_HELPERS_CODE);
  js.push('');

  // Format helpers
  js.push(FORMAT_HELPERS_CODE);
  js.push('');

  // DOM helpers
  js.push('function setText(id, text) { var el = document.getElementById(id); if (el) el.textContent = text; }');
  js.push('function setValueInElement(parentId, text) { var p = document.getElementById(parentId); if (!p) return; var v = p.querySelector(".value"); if (v) v.textContent = text; }');
  js.push('');

  // Initialize sparklines
  var sparkInits = [];
  var netSparkInits = [];
  for (var i = 0; i < manifest.widgets.length; i++) {
    var w = manifest.widgets[i];
    if (w.type === 'metric-card' && w.config.showSparkline) {
      sparkInits.push({ id: w.id, varName: 'spark_' + sanitizeId(w.id) });
    }
    if (w.type === 'sparkline') {
      var pts = w.config.maxPoints || 120;
      sparkInits.push({ id: w.id, varName: 'spark_' + sanitizeId(w.id), maxPoints: pts });
    }
    if (w.type === 'network-pair' && w.config.showSparkline) {
      netSparkInits.push({ id: w.id, varName: 'netSpark_' + sanitizeId(w.id), maxPoints: w.config.maxPoints || 120 });
    }
  }

  js.push('var MAX_POINTS = 120;');
  for (var si = 0; si < sparkInits.length; si++) {
    var sp = sparkInits[si];
    var pts = sp.maxPoints || 'MAX_POINTS';
    js.push('var ' + sp.varName + ' = createSparkline("spark-' + sp.id + '", ' + pts + ', STATE_COLORS.normal.stroke);');
  }
  for (var ni = 0; ni < netSparkInits.length; ni++) {
    var np = netSparkInits[ni];
    js.push('var ' + np.varName + ' = createNetSparkline("spark-' + np.id + '", ' + np.maxPoints + ');');
  }
  js.push('');

  // Generate per-widget update functions
  for (var wi = 0; wi < manifest.widgets.length; wi++) {
    var widget = manifest.widgets[wi];
    js.push(compileWidgetUpdateJs(widget));
    js.push('');
  }

  // Main updateUI
  js.push('function updateUI(data) {');
  js.push('  try {');
  for (var ui = 0; ui < manifest.widgets.length; ui++) {
    var uw = manifest.widgets[ui];
    if (uw.type !== 'label' && uw.type !== 'divider') {
      js.push('    update_' + sanitizeId(uw.id) + '(data);');
    }
  }
  js.push('  } catch (e) { if (typeof console !== "undefined") console.warn("[updateUI]", e); }');
  js.push('}');
  js.push('');

  // Clock intervals
  var clockWidgets = manifest.widgets.filter(function (w) { return w.type === 'clock'; });
  if (clockWidgets.length > 0) {
    js.push('function updateClocks() {');
    js.push('  var now = new Date();');
    for (var ci = 0; ci < clockWidgets.length; ci++) {
      var cw = clockWidgets[ci];
      var cfg = cw.config;
      var h12 = cfg.format24h ? 'false' : 'true';
      var secs = cfg.showSeconds ? ", second: '2-digit'" : '';
      js.push('  var timeEl_' + ci + ' = document.getElementById("clock-time-' + cw.id + '");');
      js.push('  if (timeEl_' + ci + ') timeEl_' + ci + '.textContent = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit"' + secs + ', hour12: ' + h12 + ' });');
      if (cfg.showDate) {
        js.push('  var dateEl_' + ci + ' = document.getElementById("clock-date-' + cw.id + '");');
        js.push('  if (dateEl_' + ci + ') dateEl_' + ci + '.textContent = now.toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" });');
      }
    }
    js.push('}');
    js.push('updateClocks();');
    js.push('setInterval(updateClocks, 1000);');
    js.push('');
  }

  // Tauri event listener
  js.push(TAURI_LISTENER_CODE);

  return js.join('\n');
}

function compileWidgetUpdateJs(w) {
  var fn = 'function update_' + sanitizeId(w.id) + '(data) {';
  var lines = [fn];

  switch (w.type) {
    case 'metric-card':
      lines = lines.concat(compileMetricCardUpdateJs(w));
      break;
    case 'value-display':
      lines = lines.concat(compileValueDisplayUpdateJs(w));
      break;
    case 'progress-bar':
      lines = lines.concat(compileProgressBarUpdateJs(w));
      break;
    case 'sparkline':
      lines = lines.concat(compileSparklineUpdateJs(w));
      break;
    case 'clock':
      // Clock updates via interval, not sensor data
      break;
    case 'network-pair':
      lines = lines.concat(compileNetworkPairUpdateJs(w));
      break;
    default:
      break;
  }

  lines.push('}');
  return lines.join('\n');
}

function compileMetricCardUpdateJs(w) {
  var c = w.config;
  var sparkVar = 'spark_' + sanitizeId(w.id);
  var lines = [];

  // Auto-migrate legacy progress/sparkline fields to use percentages (local only, no mutation)
  var progressField = c.progressField;
  var sparklineField = c.sparklineField;
  if (progressField === 'ram_used') progressField = 'ram_usage';
  if (progressField === 'disk_used') progressField = 'disk_usage';
  if (sparklineField === 'ram_used') sparklineField = 'ram_usage';
  if (sparklineField === 'disk_used') sparklineField = 'disk_usage';
  // Validate optional fields only if present
  if (progressField) assertValidField(progressField);
  if (sparklineField) assertValidField(sparklineField);

  function getFormatExpr(fieldVar, fieldName) {
    if (fieldName.indexOf('net_') !== -1) return 'formatRate(' + fieldVar + ').value';
    if (fieldName.indexOf('_used') !== -1 || fieldName.indexOf('_total') !== -1) return 'formatBytes(' + fieldVar + ')';
    return fieldVar + '.toFixed(0)';
  }

  // Primary value
  var primaryField = assertValidField(c.primaryField);
  lines.push('  var pVal = data.' + primaryField + ';');
  lines.push('  if (pVal != null) setValueInElement("primary-' + w.id + '", ' + getFormatExpr('pVal', primaryField) + ');');

  // Secondary fields
  if (c.secondaryFields) {
    for (var i = 0; i < c.secondaryFields.length; i++) {
      var sf = c.secondaryFields[i];
      var sfField = assertValidField(sf.field);
      lines.push('  var secVal' + i + ' = data.' + sfField + ';');
      lines.push('  if (secVal' + i + ' != null) setText("sec-' + i + '-' + w.id + '", ' + getFormatExpr('secVal' + i, sfField) + ');');
    }
  }

  // State
  var threshField = primaryField;
  var threshFn = 'getLoadState';
  if (threshField.indexOf('temp') !== -1) threshFn = 'getTemperatureState';
  if (threshField.indexOf('disk') !== -1) threshFn = 'getDiskState';

  lines.push('  var state = ' + threshFn + '(data.' + (progressField || primaryField) + ');');
  lines.push('  applyState("card-' + w.id + '", state);');

  // Progress bar
  if (c.showProgress && progressField) {
    lines.push('  var barVal = data.' + progressField + ';');
    lines.push('  if (barVal != null) {');
    lines.push('    var bar = document.getElementById("bar-' + w.id + '");');
    lines.push('    if (bar) bar.style.width = barVal.toFixed(0) + "%";');
    lines.push('    setText("bar-label-' + w.id + '", barVal.toFixed(0) + "%");');
    lines.push('  }');
  }

  // Sparkline
  if (c.showSparkline && sparklineField) {
    lines.push('  var sparkVal = data.' + sparklineField + ';');
    lines.push('  if (sparkVal != null && ' + sparkVar + ') {');
    lines.push('    ' + sparkVar + '.push(sparkVal);');
    lines.push('    var colors = STATE_COLORS[state];');
    lines.push('    ' + sparkVar + '.setColors(colors.stroke);');
    lines.push('    ' + sparkVar + '.draw();');
    lines.push('  }');
  }

  return lines;
}

function compileValueDisplayUpdateJs(w) {
  var c = w.config;
  var lines = [];
  lines.push('  var val = data.' + assertValidField(c.field) + ';');
  if (c.formatAsBytes) {
    lines.push('  if (val != null) setValueInElement("val-' + w.id + '", formatBytes(val));');
  } else {
    lines.push('  if (val != null) setValueInElement("val-' + w.id + '", val.toFixed(0));');
  }
  return lines;
}

function compileProgressBarUpdateJs(w) {
  var c = w.config;
  var lines = [];
  lines.push('  var val = data.' + assertValidField(c.field) + ';');
  lines.push('  if (val != null) {');
  lines.push('    var bar = document.getElementById("bar-' + w.id + '");');
  lines.push('    if (bar) bar.style.width = val.toFixed(0) + "%";');
  if (c.showLabel) {
    lines.push('    setText("bar-label-' + w.id + '", val.toFixed(0) + "%");');
  }
  lines.push('  }');
  return lines;
}

function compileSparklineUpdateJs(w) {
  var sparkVar = 'spark_' + sanitizeId(w.id);
  var lines = [];
  lines.push('  var val = data.' + assertValidField(w.config.field) + ';');
  lines.push('  if (val != null && ' + sparkVar + ') {');
  lines.push('    ' + sparkVar + '.push(val);');
  lines.push('    ' + sparkVar + '.draw();');
  lines.push('  }');
  return lines;
}

function compileNetworkPairUpdateJs(w) {
  var lines = [];
  var sparkVar = 'netSpark_' + sanitizeId(w.id);

  lines.push('  var upEl = document.getElementById("net-up-' + w.id + '");');
  lines.push('  var downEl = document.getElementById("net-down-' + w.id + '");');
  lines.push('  if (data.net_upload != null && upEl) {');
  lines.push('    var up = formatRate(data.net_upload);');
  lines.push('    upEl.querySelector(".value").textContent = up.value;');
  lines.push('    upEl.querySelector(".unit").textContent = up.unit;');
  lines.push('  }');
  lines.push('  if (data.net_download != null && downEl) {');
  lines.push('    var dn = formatRate(data.net_download);');
  lines.push('    downEl.querySelector(".value").textContent = dn.value;');
  lines.push('    downEl.querySelector(".unit").textContent = dn.unit;');
  lines.push('  }');

  if (w.config.showSparkline) {
    lines.push('  if (' + sparkVar + ') {');
    lines.push('    ' + sparkVar + '.push(');
    lines.push('      data.net_upload != null ? data.net_upload / 1024 : 0,');
    lines.push('      data.net_download != null ? data.net_download / 1024 : 0');
    lines.push('    );');
    lines.push('    ' + sparkVar + '.draw();');
    lines.push('  }');
  }

  return lines;
}


// ── Shared Code Templates ─────────────────────────────────────

var SPARKLINE_FACTORY_CODE = [
  'function createSparkline(canvasId, maxPoints, strokeColor) {',
  '  var canvas = document.getElementById(canvasId);',
  '  if (!canvas) return null;',
  '  var ctx = canvas.getContext("2d");',
  '  var data = [];',
  '  var maxPts = maxPoints || 120;',
  '  function resize() {',
  '    var rect = canvas.parentElement.getBoundingClientRect();',
  '    var dpr = window.devicePixelRatio || 1;',
  '    canvas.width = rect.width * dpr;',
  '    canvas.height = rect.height * dpr;',
  '    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);',
  '  }',
  '  resize();',
  '  return {',
  '    push: function(value) { data.push(value); if (data.length > maxPts) data.shift(); },',
  '    setColors: function(s) { strokeColor = s; },',
  '    draw: function() {',
  '      var w = canvas.width / (window.devicePixelRatio || 1);',
  '      var h = canvas.height / (window.devicePixelRatio || 1);',
  '      ctx.clearRect(0, 0, w, h);',
  '      if (data.length < 2) return;',
  '      var max = 0;',
  '      for (var i = 0; i < data.length; i++) { if (data[i] > max) max = data[i]; }',
  '      if (max < 1) max = 1;',
  '      max = max * 1.15;',
  '      var stepX = w / (maxPts - 1);',
  '      var startX = w - (data.length - 1) * stepX;',
  '      var padBottom = 1;',
  '      ctx.beginPath();',
  '      ctx.moveTo(startX, h - padBottom - ((data[0] / max) * (h - padBottom - 1)));',
  '      for (var j = 1; j < data.length; j++) {',
  '        ctx.lineTo(startX + j * stepX, h - padBottom - ((data[j] / max) * (h - padBottom - 1)));',
  '      }',
  '      ctx.strokeStyle = strokeColor;',
  '      ctx.lineWidth = 1.2;',
  '      ctx.lineJoin = "round";',
  '      ctx.stroke();',
  '    },',
  '    resize: resize',
  '  };',
  '}'
].join('\n');

var NET_SPARKLINE_FACTORY_CODE = [
  'function createNetSparkline(canvasId, maxPoints) {',
  '  var canvas = document.getElementById(canvasId);',
  '  if (!canvas) return null;',
  '  var ctx = canvas.getContext("2d");',
  '  var upData = [], downData = [];',
  '  var maxPts = maxPoints || 120;',
  '  function resize() {',
  '    var rect = canvas.parentElement.getBoundingClientRect();',
  '    var dpr = window.devicePixelRatio || 1;',
  '    canvas.width = rect.width * dpr;',
  '    canvas.height = rect.height * dpr;',
  '    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);',
  '  }',
  '  resize();',
  '  function drawArea(arr, strokeCol, w, h, max, stepX) {',
  '    if (arr.length < 2) return;',
  '    var startX = w - (arr.length - 1) * stepX;',
  '    ctx.beginPath();',
  '    ctx.moveTo(startX, h - 1 - ((arr[0] / max) * (h - 2)));',
  '    for (var j = 1; j < arr.length; j++) {',
  '      ctx.lineTo(startX + j * stepX, h - 1 - ((arr[j] / max) * (h - 2)));',
  '    }',
  '    ctx.strokeStyle = strokeCol; ctx.lineWidth = 1; ctx.lineJoin = "round"; ctx.stroke();',
  '  }',
  '  return {',
  '    push: function(up, down) {',
  '      upData.push(up); downData.push(down);',
  '      if (upData.length > maxPts) upData.shift();',
  '      if (downData.length > maxPts) downData.shift();',
  '    },',
  '    draw: function() {',
  '      var w = canvas.width / (window.devicePixelRatio || 1);',
  '      var h = canvas.height / (window.devicePixelRatio || 1);',
  '      ctx.clearRect(0, 0, w, h);',
  '      var max = 1;',
  '      for (var i = 0; i < upData.length; i++) { if (upData[i] > max) max = upData[i]; }',
  '      for (var k = 0; k < downData.length; k++) { if (downData[k] > max) max = downData[k]; }',
  '      max *= 1.2;',
  '      var stepX = w / (maxPts - 1);',
  '      drawArea(downData, "rgba(59,130,246,0.7)", w, h, max, stepX);',
  '      drawArea(upData, "rgba(34,197,94,0.7)", w, h, max, stepX);',
  '    },',
  '    resize: resize',
  '  };',
  '}'
].join('\n');

var STATE_HELPERS_CODE = [
  'var STATE_COLORS = {',
  '  normal:   { stroke: "rgba(34,197,94,0.8)",  fill: "rgba(34,197,94,0.08)" },',
  '  warning:  { stroke: "rgba(245,158,11,0.8)", fill: "rgba(245,158,11,0.08)" },',
  '  critical: { stroke: "rgba(239,68,68,0.8)",  fill: "rgba(239,68,68,0.08)" }',
  '};',
  'function getTemperatureState(t) { if (t == null) return "normal"; if (t >= 80) return "critical"; if (t >= 60) return "warning"; return "normal"; }',
  'function getLoadState(p) { if (p == null) return "normal"; if (p >= 90) return "critical"; if (p >= 70) return "warning"; return "normal"; }',
  'function getDiskState(p) { if (p == null) return "normal"; if (p >= 95) return "critical"; if (p >= 80) return "warning"; return "normal"; }',
  'function applyState(cardId, state) {',
  '  var card = document.getElementById(cardId);',
  '  if (!card) return;',
  '  card.classList.remove("state-normal", "state-warning", "state-critical");',
  '  card.classList.add("state-" + state);',
  '}'
].join('\n');

var FORMAT_HELPERS_CODE = [
  'function formatBytes(bytes) {',
  '  var gb = bytes / (1024 * 1024 * 1024);',
  '  if (gb >= 1) return gb.toFixed(1);',
  '  return (bytes / (1024 * 1024)).toFixed(0);',
  '}',
  'function formatBytesUnit(bytes) {',
  '  return bytes / (1024 * 1024 * 1024) >= 1 ? "GB" : "MB";',
  '}',
  'function formatRate(bytesPerSec) {',
  '  if (bytesPerSec >= 1024 * 1024) return { value: (bytesPerSec / (1024 * 1024)).toFixed(1), unit: "MB/s" };',
  '  return { value: (bytesPerSec / 1024).toFixed(1), unit: "KB/s" };',
  '}'
].join('\n');

var TAURI_LISTENER_CODE = [
  'function initTauriListener(attempt) {',
  '  if (attempt === undefined) attempt = 0;',
  '  if (window.__TAURI__ && window.__TAURI__.event) {',
  '    window.__TAURI__.event.listen("sensor-update", function(event) { updateUI(event.payload); })',
  '      .then(function(unsub) { window.addEventListener("beforeunload", unsub); });',
  '  } else if (attempt < 50) {',
  '    setTimeout(function() { initTauriListener(attempt + 1); }, 100);',
  '  }',
  '}',
  'if (document.readyState === "complete") { initTauriListener(); }',
  'else { window.addEventListener("load", initTauriListener); }'
].join('\n');


// ── Utility Functions ─────────────────────────────────────────

function escHtml(str) {
  if (str == null) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

function sanitizeId(id) {
  return id.replace(/[^a-zA-Z0-9_]/g, '_');
}

// Validate a sensor field name against the known allowlist to prevent code injection
// in generated JS. Returns the field name if valid, throws otherwise.
function assertValidField(name) {
  if (!name || typeof name !== 'string') throw new Error('Missing field name');
  var valid = SENSOR_FIELDS.some(function(sf) { return sf.value === name; });
  if (!valid) throw new Error('Invalid sensor field name: ' + name);
  return name;
}

function hexToRgba(hex, alpha) {
  if (!hex || hex.charAt(0) !== '#') return hex;
  var r = parseInt(hex.slice(1, 3), 16);
  var g = parseInt(hex.slice(3, 5), 16);
  var b = parseInt(hex.slice(5, 7), 16);
  if (isNaN(r)) return hex;
  return 'rgba(' + r + ', ' + g + ', ' + b + ', ' + alpha + ')';
}
