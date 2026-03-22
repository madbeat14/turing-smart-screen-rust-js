# Turing Smart Screen — Rust + JS Rewrite Design

## Overview

Rewrite the current Python-based system monitor into a **Tauri application** where:
- **JavaScript/HTML/CSS** renders the monitor UI in a hidden webview
- **Rust backend** handles sensors, serial communication, and screenshots
- The webview is periodically screenshotted, converted to RGB565, and pushed over serial to the display

### Why Rewrite?

| Concern | Python (current) | Rust + JS (proposed) |
|---------|-----------------|----------------------|
| Startup time | ~15s (PyInstaller + .NET CLR + reset) | ~3s (native binary + reset) |
| Binary size | ~80MB (bundled Python + deps) | ~10-15MB (native + webview) |
| UI flexibility | PIL text/image drawing (limited) | Full HTML/CSS/JS (unlimited) |
| Packaging | PyInstaller single-exe extraction | Native `.exe`, no extraction |
| Sensor access | LHM via pythonnet (.NET CLR) | `sysinfo` + NVML (native) |
| Serial performance | pyserial + GIL | `serialport` crate, zero-overhead |

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    Tauri App                          │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  Hidden Webview (JS/HTML/CSS)                  │  │
│  │                                                │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐   │  │
│  │  │ CPU Card │ │ GPU Card │ │ Memory Card  │   │  │
│  │  │ 45°C     │ │ 62°C     │ │ 8.2/16 GB    │   │  │
│  │  │ 3.8 GHz  │ │ 75% Load │ │ ████░░ 51%   │   │  │
│  │  └──────────┘ └──────────┘ └──────────────┘   │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────────┐   │  │
│  │  │ Disk     │ │ Network  │ │ Date / Time  │   │  │
│  │  │ 512 GB   │ │ ↑12 ↓45  │ │ Mar 21 19:00 │   │  │
│  │  └──────────┘ └──────────┘ └──────────────┘   │  │
│  └────────────────────────────────────────────────┘  │
│                        │ screenshot (1-2 FPS)         │
│                        ▼                              │
│  ┌────────────────────────────────────────────────┐  │
│  │  Rust Backend                                  │  │
│  │                                                │  │
│  │  ┌─────────────┐  ┌──────────────────────┐    │  │
│  │  │ Sensor      │  │ Display Driver       │    │  │
│  │  │ Manager     │  │                      │    │  │
│  │  │             │  │  screenshot → resize  │    │  │
│  │  │ - sysinfo   │  │  → RGB565 convert    │    │  │
│  │  │ - nvml      │  │  → chunk + serial TX │    │  │
│  │  │ - wmi       │  │                      │    │  │
│  │  └──────┬──────┘  └──────────┬───────────┘    │  │
│  │         │                    │                 │  │
│  │         │ invoke()           │ serial write    │  │
│  │         ▼                    ▼                 │  │
│  │  ┌─────────────┐  ┌──────────────────────┐    │  │
│  │  │ JS Webview  │  │ USB Serial (COM Port)│    │  │
│  │  │ (updates UI)│  │ 115200 baud          │    │  │
│  │  └─────────────┘  └──────────────────────┘    │  │
│  └────────────────────────────────────────────────┘  │
│                                                      │
│  ┌────────────────────────────────────────────────┐  │
│  │  System Tray (tray-icon crate)                 │  │
│  │  ├─ Configure (opens settings webview)         │  │
│  │  └─ Exit                                       │  │
│  └────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────┘
```

---

## Data Flow

```
┌───────────┐    invoke()     ┌──────────┐   update DOM   ┌──────────┐
│  Rust     │ ──────────────► │    JS    │ ─────────────► │  Webview │
│  Sensors  │  (sensor data)  │  Bridge  │   (live data)  │  Render  │
└───────────┘                 └──────────┘                └────┬─────┘
                                                               │
                                                    screenshot (PNG/raw)
                                                               │
┌───────────┐   serial write  ┌──────────┐    RGB565     ┌────▼─────┐
│  Turing   │ ◄────────────── │  Serial  │ ◄──────────── │  Image   │
│  Display  │   (chunked)     │  Driver  │  (conversion) │  Process │
└───────────┘                 └──────────┘               └──────────┘
```

### Loop (runs every 500ms–1s):

1. **Rust** reads sensors via `sysinfo`/NVML/WMI
2. **Rust → JS**: pushes sensor data to the webview via `tauri::Manager::emit()`
3. **JS** updates the DOM (text, progress bars, charts)
4. **Rust** takes a screenshot of the webview → raw RGBA pixels
5. **Rust** resizes to display resolution (e.g. 480×320), converts RGBA → RGB565
6. **Rust** diffs against previous frame (optional optimization)
7. **Rust** sends changed regions over serial to the Turing display

### Current Python Implementation — Loop Timing Estimate

The Python implementation doesn't use a single unified loop. Instead, it runs **~15 separate scheduled threads** (one per sensor), each on their own interval (typically 1–5s), all feeding updates into a shared `update_queue` drained every **10ms** by the `QueueHandler`. Each update renders a PIL image per widget, converts to RGB565 via numpy, then sends over serial.

#### Per-Step Comparison

| Step | Rust (proposed) | Python (current) | Estimated Python Time |
|------|----------------|------------------|----------------------|
| **1. Read sensors** | `sysinfo`/NVML/WMI (native, ~1–5ms) | LHM via pythonnet (.NET CLR interop) | **5–30ms** per sensor call |
| **2. Push data to UI** | `emit()` IPC (~0ms) | N/A — renders directly, no separate UI | **0ms** |
| **3. Update DOM** | JS DOM update (~1ms) | N/A | **0ms** |
| **4. Screenshot** | Win32 `PrintWindow` (~1–2ms) | N/A — draws directly with PIL | **0ms** |
| **5. Resize + RGB565** | `image` crate (~1ms) | PIL `Image.new()` + `ImageDraw` + numpy RGB565 | **2–10ms per widget** (~30–100ms total for 10–20 widgets) |
| **6. Frame diff** | Tile-based diff (~0.5ms) | **None** — sends full widget region every time | **0ms** (wastes serial bandwidth) |
| **7. Serial TX** | Chunked serial write | pyserial + queue, 115200 baud | **~27ms** per small text; full-screen = **~27s** |

#### Total Per Visible Refresh (typical, ~5–10 values change)

| Component | Python | Rust |
|-----------|--------|------|
| Sensor reads | ~50–150ms | ~1–5ms |
| Rendering | ~30–100ms (PIL per widget) | ~1–2ms (DOM update) |
| Image processing | ~5–20ms (numpy) | ~1–2ms (resize + RGB565) |
| Serial TX | ~50–200ms | ~50–200ms (same HW limit) |
| Queue/GIL overhead | ~10–30ms | 0ms |
| **Total** | **~150–500ms** | **~55–210ms** |

#### Key Takeaway

Serial bandwidth is the **same bottleneck** in both (~11.5 KB/s at 115200 baud). The big wins from Rust:

1. **Sensor reading** ~10–30× faster (no .NET CLR interop)
2. **Rendering** effectively free (browser engine vs PIL image creation per widget)
3. **No GIL** — all threads run truly parallel
4. **Frame diffing** could cut serial traffic by 50–90%

Effective refresh latency: **~150–500ms → ~55–210ms**, and with frame diffing, typical sensor changes could update within **~50–100ms**.

---

## Project Structure

```
turing-smart-screen-rs/
├── src-tauri/
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # Entry point, Tauri setup
│   │   ├── sensors/
│   │   │   ├── mod.rs           # Sensor trait + manager
│   │   │   ├── cpu.rs           # CPU stats (sysinfo)
│   │   │   ├── gpu.rs           # GPU stats (nvml-wrapper)
│   │   │   ├── memory.rs        # RAM stats (sysinfo)
│   │   │   ├── disk.rs          # Disk stats (sysinfo)
│   │   │   ├── network.rs       # Network stats (sysinfo)
│   │   │   └── weather.rs       # Weather API (reqwest)
│   │   ├── display/
│   │   │   ├── mod.rs           # Display trait
│   │   │   ├── serial.rs        # Serial port comm (serialport crate)
│   │   │   ├── protocol_a.rs    # Rev A command protocol
│   │   │   ├── protocol_b.rs    # Rev B protocol
│   │   │   ├── rgb565.rs        # RGBA → RGB565 converter
│   │   │   └── diff.rs          # Frame diff (send only changed regions)
│   │   ├── screenshot.rs        # Webview screenshot capture
│   │   ├── config.rs            # YAML config (serde)
│   │   └── tray.rs              # System tray setup
│   ├── tauri.conf.json
│   └── icons/
├── src/                          # JS/HTML/CSS frontend
│   ├── index.html               # Monitor layout
│   ├── style.css                # Theme styling
│   ├── app.js                   # Sensor data listener + DOM updates
│   ├── themes/
│   │   ├── default/
│   │   │   ├── theme.css        # Default theme
│   │   │   └── layout.html      # Default layout
│   │   ├── minimal/
│   │   └── gaming/
│   └── components/
│       ├── cpu-card.js           # CPU widget
│       ├── gpu-card.js           # GPU widget
│       ├── memory-bar.js         # Memory progress bar
│       ├── disk-usage.js         # Disk widget
│       ├── network-graph.js      # Network line chart
│       └── clock.js              # Date/time widget
├── configure/                    # Settings UI (separate Tauri window)
│   ├── index.html
│   └── settings.js
├── package.json
└── README.md
```

---

## Rust Crates

| Crate | Purpose |
|-------|---------|
| `tauri` | App framework, webview, IPC, tray |
| `serialport` | Serial COM port communication |
| `sysinfo` | CPU, RAM, disk, network stats |
| `nvml-wrapper` | NVIDIA GPU stats (temp, load, VRAM) |
| `image` | Image resizing, pixel manipulation |
| `serde` / `serde_yaml` | Config file parsing |
| `tokio` | Async runtime for sensor + serial loops |
| `tray-icon` | System tray (bundled with Tauri v2) |
| `wmi` | Windows Management Instrumentation (fan speed, etc.) |
| `reqwest` | HTTP client for weather API |

---

## Key Implementation Details

### 1. Screenshot Capture

Tauri v2 doesn't have a built-in screenshot API. Options:

```rust
// Option A: Use wry's webview to render offscreen and capture
// This requires a custom wry setup outside Tauri

// Option B: Use the webview's JavaScript to capture via html2canvas
// Then pass the base64 image back to Rust
tauri::command]
async fn capture_frame(window: tauri::Window) -> Result<Vec<u8>, String> {
    // Execute JS in webview to capture canvas
    let result = window.eval("captureFrame()").await?;
    // decode base64 PNG → raw pixels
    Ok(pixels)
}

// Option C: Use Windows API (BitBlt) to capture the hidden window
// Most reliable, works for any webview content
```

**Recommended: Option C** — Use Win32 `PrintWindow` or `BitBlt` on the hidden Tauri window. Most reliable, no JS overhead.

### 2. RGB565 Conversion

```rust
/// Convert RGBA pixel buffer to RGB565 little-endian
fn rgba_to_rgb565_le(rgba: &[u8]) -> Vec<u8> {
    let mut rgb565 = Vec::with_capacity(rgba.len() / 2);
    for pixel in rgba.chunks_exact(4) {
        let r = (pixel[0] >> 3) as u16;
        let g = (pixel[1] >> 2) as u16;
        let b = (pixel[2] >> 3) as u16;
        let color: u16 = (r << 11) | (g << 5) | b;
        rgb565.extend_from_slice(&color.to_le_bytes());
    }
    rgb565
}
```

### 3. Frame Diffing (Optional Optimization)

Instead of sending the entire 480×320 frame every cycle, compare against the previous frame and only send changed rectangular regions:

```rust
fn diff_frames(prev: &[u16], curr: &[u16], width: usize) -> Vec<DirtyRect> {
    // Divide screen into tiles (e.g. 32x32)
    // Compare each tile, collect changed tiles
    // Merge adjacent dirty tiles into larger rectangles
    // Return list of regions to update
}
```

This is crucial since serial bandwidth is only ~11.5 KB/s at 115200 baud.

### 4. Sensor Loop

```rust
async fn sensor_loop(app_handle: AppHandle) {
    let mut sys = System::new_all();
    loop {
        sys.refresh_all();
        let data = SensorData {
            cpu_temp: get_cpu_temp(&sys),
            cpu_freq: get_cpu_freq(&sys),
            cpu_usage: sys.global_cpu_usage(),
            ram_used: sys.used_memory(),
            ram_total: sys.total_memory(),
            // ... etc
        };
        app_handle.emit("sensor-update", &data).ok();
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
```

### 5. JS Side (Receiving Sensor Data)

```javascript
import { listen } from '@tauri-apps/api/event';

listen('sensor-update', (event) => {
    const data = event.payload;
    document.getElementById('cpu-temp').textContent = `${data.cpu_temp}°C`;
    document.getElementById('cpu-bar').style.width = `${data.cpu_usage}%`;
    document.getElementById('ram-text').textContent =
        `${(data.ram_used / 1e9).toFixed(1)} / ${(data.ram_total / 1e9).toFixed(1)} GB`;
    // ... update all widgets
});
```

---

## Theming System

Themes are pure HTML/CSS — users can create custom themes without any code:

```
themes/
├── default/
│   ├── layout.html    ← Widget positions, structure
│   ├── theme.css      ← Colors, fonts, animations
│   └── assets/        ← Background images, icons
├── gaming/
│   ├── layout.html
│   ├── theme.css
│   └── assets/
└── minimal/
```

Theme CSS example:
```css
/* themes/gaming/theme.css */
:root {
    --bg: #0d1117;
    --accent: #58a6ff;
    --danger: #f85149;
    --success: #3fb950;
    --font: 'Inter', sans-serif;
}

body {
    width: 480px;       /* Match display resolution */
    height: 320px;
    background: var(--bg);
    font-family: var(--font);
    overflow: hidden;    /* No scrollbars */
}

.sensor-card {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    backdrop-filter: blur(10px);
}

.progress-bar {
    background: linear-gradient(90deg, var(--success), var(--danger));
    transition: width 0.3s ease;
}
```

> **Advantage over Python**: Glassmorphism, gradients, animations, custom fonts, SVG icons — all native in CSS. No PIL hacks needed.

---

## Migration Path

| Phase | Work | Effort |
|-------|------|--------|
| **Phase 1** | Rust serial driver (Rev A protocol) | 1-2 days |
| **Phase 2** | Sensor reading (`sysinfo` + NVML) | 1-2 days |
| **Phase 3** | Tauri app + hidden webview + screenshot loop | 2-3 days |
| **Phase 4** | JS monitor UI (default theme) | 1-2 days |
| **Phase 5** | Frame diffing + optimization | 1-2 days |
| **Phase 6** | Settings UI / configurator | 2-3 days |
| **Phase 7** | System tray, auto-start, installer | 1 day |

**Total estimate: ~10-15 days**

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Webview screenshot reliability | Frame capture may fail or be slow | Use Win32 `PrintWindow` API directly, not JS-based capture |
| Serial bandwidth unchanged | Still ~11.5 KB/s at 115200 baud | Frame diffing (only send changed regions) |
| GPU sensors without LHM | NVIDIA only via NVML, no AMD | Use `nvml-wrapper` for NVIDIA; AMD via WMI or `amdgpu-sysfs` |
| Theme backwards compatibility | Existing YAML themes won't work | Provide a converter tool or ship equivalent HTML/CSS themes |
| Hidden webview rendering | Some GPUs skip rendering hidden windows | Use offscreen rendering mode or `visibility: visible` with a positioned-offscreen window |
