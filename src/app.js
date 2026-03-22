// Sensor data listener — receives updates from Rust backend via Tauri IPC

function formatBytes(bytes) {
  const gb = bytes / (1024 * 1024 * 1024);
  return gb >= 1 ? gb.toFixed(1) + ' GB' : (bytes / (1024 * 1024)).toFixed(0) + ' MB';
}

function formatRate(bytesPerSec) {
  if (bytesPerSec >= 1024 * 1024) {
    return (bytesPerSec / (1024 * 1024)).toFixed(1) + ' MB/s';
  }
  return (bytesPerSec / 1024).toFixed(1) + ' KB/s';
}

function updateClock() {
  const now = new Date();
  const timeEl = document.getElementById('clock-time');
  const dateEl = document.getElementById('clock-date');
  if (timeEl) {
    timeEl.textContent = now.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit', hour12: false });
  }
  if (dateEl) {
    dateEl.textContent = now.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
  }
}

function updateUI(data) {
  // CPU
  if (data.cpu_usage != null) {
    const el = document.getElementById('cpu-usage');
    const bar = document.getElementById('cpu-bar');
    if (el) el.textContent = data.cpu_usage.toFixed(0) + '%';
    if (bar) bar.style.width = data.cpu_usage.toFixed(0) + '%';
  }
  if (data.cpu_temp != null) {
    const el = document.getElementById('cpu-temp');
    if (el) el.textContent = data.cpu_temp.toFixed(0) + '\u00B0C';
  }
  if (data.cpu_freq != null) {
    const el = document.getElementById('cpu-freq');
    if (el) el.textContent = data.cpu_freq.toFixed(0) + ' MHz';
  }

  // GPU
  if (data.gpu_usage != null) {
    const el = document.getElementById('gpu-usage');
    const bar = document.getElementById('gpu-bar');
    if (el) el.textContent = data.gpu_usage.toFixed(0) + '%';
    if (bar) bar.style.width = data.gpu_usage.toFixed(0) + '%';
  }
  if (data.gpu_temp != null) {
    const el = document.getElementById('gpu-temp');
    if (el) el.textContent = data.gpu_temp.toFixed(0) + '\u00B0C';
  }
  if (data.gpu_freq != null) {
    const el = document.getElementById('gpu-freq');
    if (el) el.textContent = data.gpu_freq.toFixed(0) + ' MHz';
  }

  // Memory
  if (data.ram_used != null && data.ram_total != null) {
    const el = document.getElementById('mem-text');
    const bar = document.getElementById('mem-bar');
    const label = document.getElementById('mem-usage');
    const pct = (data.ram_used / data.ram_total) * 100;
    if (el) el.textContent = formatBytes(data.ram_used) + ' / ' + formatBytes(data.ram_total);
    if (bar) bar.style.width = pct.toFixed(0) + '%';
    if (label) label.textContent = pct.toFixed(0) + '%';
  }

  // Disk
  if (data.disk_used != null && data.disk_total != null) {
    const el = document.getElementById('disk-text');
    const bar = document.getElementById('disk-bar');
    const label = document.getElementById('disk-usage');
    const pct = (data.disk_used / data.disk_total) * 100;
    if (el) el.textContent = formatBytes(data.disk_used) + ' / ' + formatBytes(data.disk_total);
    if (bar) bar.style.width = pct.toFixed(0) + '%';
    if (label) label.textContent = pct.toFixed(0) + '%';
  }

  // Network
  if (data.net_upload != null) {
    const el = document.getElementById('net-up');
    if (el) el.textContent = '\u2191 ' + formatRate(data.net_upload);
  }
  if (data.net_download != null) {
    const el = document.getElementById('net-down');
    if (el) el.textContent = '\u2193 ' + formatRate(data.net_download);
  }
}

// Wait for Tauri API to be ready, then start listening
function initTauriListener() {
  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen('sensor-update', (event) => {
      updateUI(event.payload);
    });
    console.log('Tauri sensor listener registered');
  } else {
    // Retry until Tauri API is available
    setTimeout(initTauriListener, 100);
  }
}

// Start clock immediately
updateClock();
setInterval(updateClock, 1000);

// Start sensor listener when ready
if (document.readyState === 'complete') {
  initTauriListener();
} else {
  window.addEventListener('load', initTauriListener);
}
