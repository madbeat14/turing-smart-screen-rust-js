// Template v2 — Sparklines & State Colors

// ── Sparkline Chart (Canvas API) ──────────────────────────────

function createSparkline(canvasId, maxPoints, strokeColor) {
  var canvas = document.getElementById(canvasId);
  if (!canvas) return null;
  var ctx = canvas.getContext('2d');
  var data = [];
  var maxPts = maxPoints || 120;

  function resize() {
    var rect = canvas.parentElement.getBoundingClientRect();
    var dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  resize();

  return {
    push: function(value) {
      data.push(value);
      if (data.length > maxPts) data.shift();
    },

    setColors: function(stroke) {
      strokeColor = stroke;
    },

    draw: function() {
      var w = canvas.width / (window.devicePixelRatio || 1);
      var h = canvas.height / (window.devicePixelRatio || 1);
      ctx.clearRect(0, 0, w, h);

      if (data.length < 2) return;

      var max = 0;
      for (var i = 0; i < data.length; i++) {
        if (data[i] > max) max = data[i];
      }
      if (max < 1) max = 1;
      max = max * 1.15;

      var stepX = w / (maxPts - 1);
      var startX = w - (data.length - 1) * stepX;
      var padBottom = 1;

      ctx.beginPath();
      ctx.moveTo(startX, h - padBottom - ((data[0] / max) * (h - padBottom - 1)));
      for (var j = 1; j < data.length; j++) {
        var x = startX + j * stepX;
        var y = h - padBottom - ((data[j] / max) * (h - padBottom - 1));
        ctx.lineTo(x, y);
      }

      ctx.strokeStyle = strokeColor;
      ctx.lineWidth = 1.2;
      ctx.lineJoin = 'round';
      ctx.stroke();
    },

    resize: resize,
    getData: function() { return data; }
  };
}

function createNetSparkline(canvasId, maxPoints) {
  var canvas = document.getElementById(canvasId);
  if (!canvas) return null;
  var ctx = canvas.getContext('2d');
  var upData = [];
  var downData = [];
  var maxPts = maxPoints || 120;

  function resize() {
    var rect = canvas.parentElement.getBoundingClientRect();
    var dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  resize();

  function drawArea(dataArr, strokeCol, w, h, max, stepX) {
    if (dataArr.length < 2) return;
    var startX = w - (dataArr.length - 1) * stepX;
    var padBottom = 1;

    ctx.beginPath();
    ctx.moveTo(startX, h - padBottom - ((dataArr[0] / max) * (h - padBottom - 1)));
    for (var j = 1; j < dataArr.length; j++) {
      ctx.lineTo(startX + j * stepX, h - padBottom - ((dataArr[j] / max) * (h - padBottom - 1)));
    }
    ctx.strokeStyle = strokeCol;
    ctx.lineWidth = 1;
    ctx.lineJoin = 'round';
    ctx.stroke();
  }

  return {
    push: function(up, down) {
      upData.push(up);
      downData.push(down);
      if (upData.length > maxPts) upData.shift();
      if (downData.length > maxPts) downData.shift();
    },

    draw: function() {
      var w = canvas.width / (window.devicePixelRatio || 1);
      var h = canvas.height / (window.devicePixelRatio || 1);
      ctx.clearRect(0, 0, w, h);

      var max = 1;
      for (var i = 0; i < upData.length; i++) {
        if (upData[i] > max) max = upData[i];
      }
      for (var k = 0; k < downData.length; k++) {
        if (downData[k] > max) max = downData[k];
      }
      max = max * 1.2;

      var stepX = w / (maxPts - 1);
      drawArea(downData, 'rgba(59,130,246,0.7)', w, h, max, stepX);
      drawArea(upData, 'rgba(34,197,94,0.7)', w, h, max, stepX);
    },

    resize: resize
  };
}


// ── State Color Thresholds ───────────────────────────────────

var STATE_COLORS = {
  normal:   { stroke: 'rgba(34,197,94,0.8)',  fill: 'rgba(34,197,94,0.08)' },
  warning:  { stroke: 'rgba(245,158,11,0.8)', fill: 'rgba(245,158,11,0.08)' },
  critical: { stroke: 'rgba(239,68,68,0.8)',  fill: 'rgba(239,68,68,0.08)' }
};

function getTemperatureState(temp) {
  if (temp == null) return 'normal';
  if (temp >= 80) return 'critical';
  if (temp >= 60) return 'warning';
  return 'normal';
}

function getLoadState(pct) {
  if (pct == null) return 'normal';
  if (pct >= 90) return 'critical';
  if (pct >= 70) return 'warning';
  return 'normal';
}

function getDiskState(pct) {
  if (pct == null) return 'normal';
  if (pct >= 95) return 'critical';
  if (pct >= 80) return 'warning';
  return 'normal';
}

function applyState(cardId, state) {
  var card = document.getElementById(cardId);
  if (!card) return;
  card.classList.remove('state-normal', 'state-warning', 'state-critical');
  card.classList.add('state-' + state);
}


// ── Sparkline Instances ──────────────────────────────────────

var MAX_POINTS = 120;

var cpuSpark = createSparkline('cpu-spark', MAX_POINTS, STATE_COLORS.normal.stroke);
var gpuSpark = createSparkline('gpu-spark', MAX_POINTS, STATE_COLORS.normal.stroke);
var memSpark = createSparkline('mem-spark', MAX_POINTS, STATE_COLORS.normal.stroke);
var diskSpark = createSparkline('disk-spark', MAX_POINTS, STATE_COLORS.normal.stroke);
var netSpark = createNetSparkline('net-spark', MAX_POINTS);


// ── Format Helpers ───────────────────────────────────────────

function formatBytes(bytes) {
  var gb = bytes / (1024 * 1024 * 1024);
  if (gb >= 1) return gb.toFixed(1);
  return (bytes / (1024 * 1024)).toFixed(0);
}

function formatBytesUnit(bytes) {
  var gb = bytes / (1024 * 1024 * 1024);
  return gb >= 1 ? 'GB' : 'MB';
}

function formatRate(bytesPerSec) {
  if (bytesPerSec >= 1024 * 1024) {
    return { value: (bytesPerSec / (1024 * 1024)).toFixed(1), unit: 'MB/s' };
  }
  return { value: (bytesPerSec / 1024).toFixed(1), unit: 'KB/s' };
}


// ── DOM Update Functions ─────────────────────────────────────

function setText(id, text) {
  var el = document.getElementById(id);
  if (el) el.textContent = text;
}

function setValueInElement(parentId, text) {
  var parent = document.getElementById(parentId);
  if (!parent) return;
  var val = parent.querySelector('.value');
  if (val) val.textContent = text;
}

function updateCpuMetrics(data) {
  var temp = data.cpu_temp;
  var usage = data.cpu_usage;
  var freq = data.cpu_freq;

  var tempState = getTemperatureState(temp);
  var loadState = getLoadState(usage);
  var state = tempState === 'critical' || loadState === 'critical' ? 'critical'
    : tempState === 'warning' || loadState === 'warning' ? 'warning'
    : 'normal';
  applyState('v2-cpu-card', state);

  if (temp != null) setValueInElement('v2-cpu-temp', temp.toFixed(0));
  if (freq != null) setText('v2-cpu-freq', freq.toFixed(0));
  if (usage != null) {
    setText('v2-cpu-usage-text', usage.toFixed(0));
    setText('v2-cpu-usage-label', usage.toFixed(0) + '%');
    var bar = document.getElementById('v2-cpu-bar');
    if (bar) bar.style.width = usage.toFixed(0) + '%';
  }

  if (usage != null && cpuSpark) {
    cpuSpark.push(usage);
    var colors = STATE_COLORS[state];
    cpuSpark.setColors(colors.stroke);
    cpuSpark.draw();
  }
}

function updateGpuMetrics(data) {
  var temp = data.gpu_temp;
  var usage = data.gpu_usage;
  var freq = data.gpu_freq;

  var tempState = getTemperatureState(temp);
  var loadState = getLoadState(usage);
  var state = tempState === 'critical' || loadState === 'critical' ? 'critical'
    : tempState === 'warning' || loadState === 'warning' ? 'warning'
    : 'normal';
  applyState('v2-gpu-card', state);

  if (temp != null) setValueInElement('v2-gpu-temp', temp.toFixed(0));
  if (freq != null) setText('v2-gpu-freq', freq.toFixed(0));
  if (usage != null) {
    setText('v2-gpu-usage-text', usage.toFixed(0));
    setText('v2-gpu-usage-label', usage.toFixed(0) + '%');
    var bar = document.getElementById('v2-gpu-bar');
    if (bar) bar.style.width = usage.toFixed(0) + '%';
  }

  if (usage != null && gpuSpark) {
    gpuSpark.push(usage);
    var colors = STATE_COLORS[state];
    gpuSpark.setColors(colors.stroke);
    gpuSpark.draw();
  }
}

function updateMemoryMetrics(data) {
  if (data.ram_used == null || data.ram_total == null) return;

  var pct = (data.ram_used / data.ram_total) * 100;
  var state = getLoadState(pct);
  applyState('v2-mem-card', state);

  setValueInElement('v2-mem-pct', pct.toFixed(0));

  var memText = document.getElementById('v2-mem-text');
  if (memText) {
    var usedVal = formatBytes(data.ram_used);
    var totalVal = formatBytes(data.ram_total);
    var unit = formatBytesUnit(data.ram_total);
    memText.innerHTML =
      '<span class="value">' + usedVal + '</span>' +
      '<span class="unit"> / </span>' +
      '<span class="value">' + totalVal + '</span>' +
      '<span class="unit">' + unit + '</span>';
  }

  setText('v2-mem-usage-label', pct.toFixed(0) + '%');
  var bar = document.getElementById('v2-mem-bar');
  if (bar) bar.style.width = pct.toFixed(0) + '%';

  if (memSpark) {
    memSpark.push(pct);
    var colors = STATE_COLORS[state];
    memSpark.setColors(colors.stroke);
    memSpark.draw();
  }
}

function updateDiskMetrics(data) {
  if (data.disk_used == null || data.disk_total == null) return;

  var pct = (data.disk_used / data.disk_total) * 100;
  var state = getDiskState(pct);
  applyState('v2-disk-card', state);

  setValueInElement('v2-disk-pct', pct.toFixed(0));

  var diskText = document.getElementById('v2-disk-text');
  if (diskText) {
    var usedVal = formatBytes(data.disk_used);
    var totalVal = formatBytes(data.disk_total);
    var unit = formatBytesUnit(data.disk_total);
    diskText.innerHTML =
      '<span class="value">' + usedVal + '</span>' +
      '<span class="unit"> / </span>' +
      '<span class="value">' + totalVal + '</span>' +
      '<span class="unit">' + unit + '</span>';
  }

  setText('v2-disk-usage-label', pct.toFixed(0) + '%');
  var bar = document.getElementById('v2-disk-bar');
  if (bar) bar.style.width = pct.toFixed(0) + '%';

  if (diskSpark) {
    diskSpark.push(pct);
    var colors = STATE_COLORS[state];
    diskSpark.setColors(colors.stroke);
    diskSpark.draw();
  }
}

function updateNetworkMetrics(data) {
  var upEl = document.getElementById('v2-net-up');
  var downEl = document.getElementById('v2-net-down');

  if (data.net_upload != null && upEl) {
    var up = formatRate(data.net_upload);
    upEl.querySelector('.value').textContent = up.value;
    upEl.querySelector('.unit').textContent = up.unit;
  }

  if (data.net_download != null && downEl) {
    var dn = formatRate(data.net_download);
    downEl.querySelector('.value').textContent = dn.value;
    downEl.querySelector('.unit').textContent = dn.unit;
  }

  if (netSpark) {
    var upBytes = data.net_upload != null ? data.net_upload / 1024 : 0;
    var dnBytes = data.net_download != null ? data.net_download / 1024 : 0;
    netSpark.push(upBytes, dnBytes);
    netSpark.draw();
  }
}

function updateClock() {
  var now = new Date();
  var timeEl = document.getElementById('v2-clock-time');
  var dateEl = document.getElementById('v2-clock-date');
  if (timeEl) {
    timeEl.textContent = now.toLocaleTimeString([], {
      hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false
    });
  }
  if (dateEl) {
    dateEl.textContent = now.toLocaleDateString([], {
      month: 'short', day: 'numeric', year: 'numeric'
    });
  }
}


// ── Main Update ──────────────────────────────────────────────

function updateUI(data) {
  updateCpuMetrics(data);
  updateGpuMetrics(data);
  updateMemoryMetrics(data);
  updateDiskMetrics(data);
  updateNetworkMetrics(data);
}


// ── Tauri Event Listener ─────────────────────────────────────

function initTauriListener(attempt) {
  if (attempt === undefined) attempt = 0;
  var MAX_ATTEMPTS = 50;

  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen('sensor-update', function(event) {
      updateUI(event.payload);
    });
  } else if (attempt < MAX_ATTEMPTS) {
    setTimeout(function() { initTauriListener(attempt + 1); }, 100);
  }
}

updateClock();
setInterval(updateClock, 1000);

if (document.readyState === 'complete') {
  initTauriListener();
} else {
  window.addEventListener('load', initTauriListener);
}
