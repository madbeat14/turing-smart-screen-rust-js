const { invoke } = window.__TAURI__.core;

const $comPort = document.getElementById('com-port');
const $revision = document.getElementById('revision');
const $brightness = document.getElementById('brightness');
const $brightnessValue = document.getElementById('brightness-value');
const $displayReverse = document.getElementById('display-reverse');
const $resetOnStartup = document.getElementById('reset-on-startup');
const $theme = document.getElementById('theme');
const $status = document.getElementById('status');

const $startOnBoot = document.getElementById('start-on-boot');

$brightness.addEventListener('input', () => {
  $brightnessValue.textContent = $brightness.value + '%';
});

// Start on Boot — applied immediately (not part of config save)
invoke('get_run_on_startup')
  .then(enabled => { $startOnBoot.checked = enabled; })
  .catch(e => showStatus('Failed to read startup setting: ' + e, true));

$startOnBoot.addEventListener('change', async () => {
  try {
    await invoke('set_run_on_startup', { enable: $startOnBoot.checked });
    showStatus($startOnBoot.checked ? 'Start on Boot enabled' : 'Start on Boot disabled', false);
  } catch (e) {
    showStatus('Failed to set startup: ' + e, true);
    $startOnBoot.checked = !$startOnBoot.checked;
  }
});

// Validate theme name: lowercase alphanumeric, hyphens, underscores only
function isValidTheme(name) {
  return /^[a-z0-9_-]{1,64}$/.test(name);
}

// Build a new config object from current form values (immutable — no mutation of source)
function buildConfigFromForm(cfg) {
  var themeValue = $theme.value.trim();
  return {
    config: {
      COM_PORT: $comPort.value,
      THEME: themeValue,
      HW_SENSORS: cfg.config.HW_SENSORS,
      ETH: cfg.config.ETH,
      WLO: cfg.config.WLO,
      CPU_FAN: cfg.config.CPU_FAN,
      PING: cfg.config.PING,
      WEATHER_API_KEY: cfg.config.WEATHER_API_KEY,
      WEATHER_LATITUDE: cfg.config.WEATHER_LATITUDE,
      WEATHER_LONGITUDE: cfg.config.WEATHER_LONGITUDE,
      WEATHER_UNITS: cfg.config.WEATHER_UNITS,
      WEATHER_LANGUAGE: cfg.config.WEATHER_LANGUAGE
    },
    display: {
      REVISION: $revision.value,
      BRIGHTNESS: parseInt($brightness.value, 10),
      DISPLAY_REVERSE: $displayReverse.checked,
      RESET_ON_STARTUP: $resetOnStartup.checked
    }
  };
}

async function refreshTemplateList() {
  const currentValue = $theme.value;
  try {
    const templates = await invoke('list_templates');
    $theme.innerHTML = '';
    for (const t of templates) {
      const opt = document.createElement('option');
      opt.value = t.name;
      opt.textContent = t.display_name + (t.is_builtin ? ' (built-in)' : '');
      $theme.appendChild(opt);
    }
    // Restore previous selection if it still exists, otherwise keep first
    if ([...($theme.options)].some(o => o.value === currentValue)) {
      $theme.value = currentValue;
    }
  } catch (e) {
    console.warn('Could not load template list:', e);
  }
}

// Listen for template list changes from editor window
window.__TAURI__.event.listen('templates-changed', function() {
  refreshTemplateList();
});

async function loadConfig() {
  try {
    // Populate COM port list
    const ports = await invoke('list_serial_ports');
    for (const port of ports) {
      const opt = document.createElement('option');
      opt.value = port;
      opt.textContent = port;
      $comPort.appendChild(opt);
    }

    await refreshTemplateList();

    // Load current config
    const cfg = await invoke('get_config');
    $comPort.value = cfg.config.COM_PORT || 'AUTO';
    $revision.value = cfg.display.REVISION || 'A';
    $brightness.value = cfg.display.BRIGHTNESS || 20;
    $brightnessValue.textContent = $brightness.value + '%';
    $displayReverse.checked = cfg.display.DISPLAY_REVERSE || false;
    $resetOnStartup.checked = cfg.display.RESET_ON_STARTUP !== false;
    $theme.value = cfg.config.THEME || 'v2';
  } catch (e) {
    showStatus('Failed to load config: ' + e, true);
  }
}

// Open Template Editor button
document.getElementById('btn-editor').addEventListener('click', async () => {
  try {
    await invoke('open_editor');
  } catch (e) {
    showStatus('Failed to open editor: ' + e, true);
  }
});

document.getElementById('btn-save').addEventListener('click', async () => {
  if (!isValidTheme($theme.value.trim())) {
    showStatus('Invalid theme name. Use only letters, numbers, hyphens, and underscores.', true);
    return;
  }
  try {
    const cfg = await invoke('get_config');
    await invoke('save_config', { newConfig: buildConfigFromForm(cfg) });
    showStatus('Settings saved. Restart to apply changes.', false);
  } catch (e) {
    showStatus('Failed to save: ' + e, true);
  }
});

document.getElementById('btn-apply').addEventListener('click', async () => {
  if (!isValidTheme($theme.value.trim())) {
    showStatus('Invalid theme name. Use only letters, numbers, hyphens, and underscores.', true);
    return;
  }
  try {
    const cfg = await invoke('get_config');
    await invoke('save_config', { newConfig: buildConfigFromForm(cfg) });
    await invoke('restart_display');
    await invoke('reload_monitor');
    showStatus('Settings saved and applied!', false);
  } catch (e) {
    showStatus('Failed to apply: ' + e, true);
  }
});

document.getElementById('btn-cancel').addEventListener('click', () => {
  window.__TAURI__.window.getCurrent().close();
});

function showStatus(msg, isError) {
  $status.textContent = msg;
  $status.className = 'status ' + (isError ? 'error' : 'success');
  if (!isError) {
    setTimeout(() => { $status.textContent = ''; }, 3000);
  }
}

// Save window size on resize (debounced)
(function() {
  var tauriWindow = window.__TAURI__.window;
  var saveTimer = null;
  window.addEventListener('resize', function() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(function() {
      var currentWindow = tauriWindow.getCurrent();
      Promise.all([
        currentWindow.outerSize(),
        currentWindow.scaleFactor()
      ]).then(function(results) {
        var size = results[0];
        var scale = results[1] || 1;
        invoke('save_window_state', {
          label: 'settings',
          state: {
            width: Math.round(size.width / scale),
            height: Math.round(size.height / scale)
          }
        }).catch(function() {});
      }).catch(function() {});
    }, 500);
  });
})();

loadConfig();
