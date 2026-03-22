# LHM Sensor Integration — Remaining Steps

## Current State

- `LhmService.exe` (headless console app) compiles and runs successfully
- It outputs JSON lines to stdout with sensor data every second
- **GPU works**: temp=36°C, usage, freq, VRAM all detected (AMD RX 6700 XT)
- **CPU temp shows 0**: sensor name matching needs fixing (AMD CPUs use different names)
- LhmService runs **without admin** and still gets GPU data
- CPU temp likely needs **admin elevation** OR the sensor name filter in `LhmService.cs` doesn't match AMD's naming

## What's Done

1. `src-tauri/external/lhm/LhmService.cs` — headless C# console app that loads `LibreHardwareMonitorLib.dll` and outputs JSON sensor data to stdout
2. `src-tauri/external/lhm/LhmService.exe` — compiled binary (net472)
3. `src-tauri/external/lhm/LibreHardwareMonitorLib.dll` + `HidSharp.dll` — LHM library
4. `src-tauri/src/lhm.rs` — Rust module that launches LhmService as hidden background process
5. `src-tauri/src/sensors/gpu.rs` — WMI-based GPU reading (works with full LHM app, NOT with our headless service)
6. `src-tauri/src/sensors/cpu.rs` — WMI-based CPU temp reading (same issue)

## What Needs to Change

### Step 1: Fix CPU temp sensor name (LhmService.cs)

Run `./LhmService.exe --dump` to see all available CPU sensors. The `--dump` flag is already implemented. Look for CPU Temperature sensors — AMD uses names like "Tctl", "Tdie", "Core (Tctl/Tdie)", or just numbered cores.

Recompile after fixing:
```bash
cd src-tauri/external/lhm
"C:/Windows/Microsoft.NET/Framework64/v4.0.30319/csc.exe" /target:exe /out:LhmService.exe /reference:LibreHardwareMonitorLib.dll /reference:HidSharp.dll LhmService.cs
```

Test: `./LhmService.exe --dump 2>/dev/null | grep -i "cpu\|temp"`

If CPU temp still shows 0 without admin, test with admin: right-click cmd → Run as Administrator → run same command.

### Step 2: Switch Rust sensor reading from WMI to stdout pipe

The current `sensors/gpu.rs` and `sensors/cpu.rs` use WMI queries (`ROOT\LibreHardwareMonitor` namespace). But our headless LhmService doesn't register WMI — it outputs JSON to stdout instead.

**Need to rewrite the integration:**

1. In `src-tauri/src/lhm.rs`:
   - After spawning LhmService, spawn a reader thread that reads stdout lines
   - Parse each JSON line into a shared `LhmSensorData` struct
   - Store in `Arc<Mutex<LhmSensorData>>` (or `Arc<RwLock<>>`)

2. In `src-tauri/src/sensors/mod.rs`:
   - Accept `LhmSensorData` reference in sensor loop
   - Merge LHM data (cpu_temp, gpu_temp, gpu_usage, gpu_freq, gpu_mem) with sysinfo data

3. Remove WMI dependency from `gpu.rs` and `cpu.rs` — no longer needed for these sensors

**JSON format from LhmService (one line per second):**
```json
{"cpu_usage":20.7,"cpu_temp":45.5,"cpu_freq":3500,"gpu_temp":36,"gpu_usage":1,"gpu_freq":74,"gpu_mem_used":3251,"gpu_mem_total":12272}
```

### Step 3: Pipe architecture

```
LhmService.exe (C#)
    ↓ stdout (JSON lines)
lhm.rs reader thread
    ↓ Arc<Mutex<LhmSensorData>>
sensors/mod.rs sensor_loop
    ↓ merges with sysinfo data
Tauri event → JS frontend
```

### Step 4: Remove WMI crate (optional cleanup)

If WMI is no longer used for sensors, remove `wmi = "0.14"` from `Cargo.toml` and simplify `gpu.rs` and `cpu.rs`.

### Step 5: Admin elevation for CPU temp

If CPU temp requires admin:
- The `launch_service_elevated()` function in `lhm.rs` already handles UAC elevation via `ShellExecuteW("runas")`
- Problem: when elevated via ShellExecuteW, we can't capture stdout (it's a separate security context)
- Solution: use a temp file or named pipe instead of stdout for the elevated case
- OR: request admin for the whole app via Windows manifest (simpler but more intrusive)

### Step 6: Copy LhmService files for release builds

For development, manually copy to `target/release/external/lhm/`:
```bash
cp src-tauri/external/lhm/LhmService.exe src-tauri/external/lhm/LibreHardwareMonitorLib.dll src-tauri/external/lhm/HidSharp.dll src-tauri/target/release/external/lhm/
```

Tauri's `bundle.resources` in `tauri.conf.json` handles this for installer builds.

## Files to Modify

| File | Change |
|------|--------|
| `external/lhm/LhmService.cs` | Fix CPU temp sensor names, recompile |
| `src/lhm.rs` | Add stdout reader thread, shared sensor state |
| `src/sensors/mod.rs` | Read from LHM shared state instead of WMI |
| `src/sensors/gpu.rs` | Simplify — remove WMI, just read from shared LHM data |
| `src/sensors/cpu.rs` | Simplify — remove WMI, just read from shared LHM data |
| `Cargo.toml` | Optionally remove `wmi` dependency |

## Test Commands

```bash
# Compile LhmService
cd src-tauri/external/lhm
"C:/Windows/Microsoft.NET/Framework64/v4.0.30319/csc.exe" /target:exe /out:LhmService.exe /reference:LibreHardwareMonitorLib.dll LhmService.cs

# Test sensor output
./LhmService.exe          # JSON lines to stdout
./LhmService.exe --dump   # All sensor names (for debugging)

# Build Rust app
cd ../.. && cargo tauri build

# Run with logging
RUST_LOG=info src-tauri/target/release/turing-smart-screen.exe
```
