// Template v1 — Simple layout

function formatBytes(bytes) {
  var gb = bytes / (1024 * 1024 * 1024);
  return gb >= 1 ? gb.toFixed(1) + ' GB' : (bytes / (1024 * 1024)).toFixed(0) + ' MB';
}

function formatRate(bytesPerSec) {
  if (bytesPerSec >= 1024 * 1024) {
    return (bytesPerSec / (1024 * 1024)).toFixed(1) + ' MB/s';
  }
  return (bytesPerSec / 1024).toFixed(1) + ' KB/s';
}

function updateClock() {
  var now = new Date();
  var timeEl = document.getElementById('v1-clock-time');
  var dateEl = document.getElementById('v1-clock-date');
  if (timeEl) {
    timeEl.textContent = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false });
  }
  if (dateEl) {
    dateEl.textContent = now.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
  }
}

function updateCpuMetrics(data) {
  if (data.cpu_usage != null) {
    var el = document.getElementById('v1-cpu-usage');
    var bar = document.getElementById('v1-cpu-bar');
    if (el) el.textContent = data.cpu_usage.toFixed(0) + '%';
    if (bar) bar.style.width = data.cpu_usage.toFixed(0) + '%';
  }
  if (data.cpu_temp != null) {
    var el = document.getElementById('v1-cpu-temp');
    if (el) el.textContent = data.cpu_temp.toFixed(0) + '\u00B0C';
  }
  if (data.cpu_freq != null) {
    var el = document.getElementById('v1-cpu-freq');
    if (el) el.textContent = data.cpu_freq.toFixed(0) + ' MHz';
  }
}

function updateGpuMetrics(data) {
  if (data.gpu_usage != null) {
    var el = document.getElementById('v1-gpu-usage');
    var bar = document.getElementById('v1-gpu-bar');
    if (el) el.textContent = data.gpu_usage.toFixed(0) + '%';
    if (bar) bar.style.width = data.gpu_usage.toFixed(0) + '%';
  }
  if (data.gpu_temp != null) {
    var el = document.getElementById('v1-gpu-temp');
    if (el) el.textContent = data.gpu_temp.toFixed(0) + '\u00B0C';
  }
  if (data.gpu_freq != null) {
    var el = document.getElementById('v1-gpu-freq');
    if (el) el.textContent = data.gpu_freq.toFixed(0) + ' MHz';
  }
}

function updateMemoryMetrics(data) {
  if (data.ram_used != null && data.ram_total != null) {
    var el = document.getElementById('v1-mem-text');
    var bar = document.getElementById('v1-mem-bar');
    var label = document.getElementById('v1-mem-usage');
    var pct = (data.ram_used / data.ram_total) * 100;
    if (el) el.textContent = formatBytes(data.ram_used) + ' / ' + formatBytes(data.ram_total);
    if (bar) bar.style.width = pct.toFixed(0) + '%';
    if (label) label.textContent = pct.toFixed(0) + '%';
  }
}

function updateDiskMetrics(data) {
  if (data.disk_used != null && data.disk_total != null) {
    var el = document.getElementById('v1-disk-text');
    var bar = document.getElementById('v1-disk-bar');
    var label = document.getElementById('v1-disk-usage');
    var pct = (data.disk_used / data.disk_total) * 100;
    if (el) el.textContent = formatBytes(data.disk_used) + ' / ' + formatBytes(data.disk_total);
    if (bar) bar.style.width = pct.toFixed(0) + '%';
    if (label) label.textContent = pct.toFixed(0) + '%';
  }
}

function updateNetworkMetrics(data) {
  if (data.net_upload != null) {
    var el = document.getElementById('v1-net-up');
    if (el) el.textContent = '\u2191 ' + formatRate(data.net_upload);
  }
  if (data.net_download != null) {
    var el = document.getElementById('v1-net-down');
    if (el) el.textContent = '\u2193 ' + formatRate(data.net_download);
  }
}

function updateUI(data) {
  updateCpuMetrics(data);
  updateGpuMetrics(data);
  updateMemoryMetrics(data);
  updateDiskMetrics(data);
  updateNetworkMetrics(data);
}

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
