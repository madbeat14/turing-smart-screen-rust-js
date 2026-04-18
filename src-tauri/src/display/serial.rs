use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, warn};
use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Signal all `SerialConnection` reconnect loops to abort and return an error.
/// Call this from the app exit handler so the display thread can exit cleanly.
pub fn signal_shutdown() {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

/// Serial port wrapper with error recovery and auto-detection
pub struct SerialConnection {
    port: Box<dyn SerialPort>,
    port_name: String,
    /// Set to true after a successful reconnect so callers can re-initialize the display
    reconnected: bool,
    /// Tracks whether the port was present in the last health check
    was_present: bool,
}

impl SerialConnection {
    /// Open a serial port with the standard Turing display settings:
    /// 115200 baud, 8N1, RTS/CTS flow control, 1s timeout
    pub fn open(port_name: &str) -> Result<Self> {
        debug!("Opening serial port: {}", port_name);
        let port = serialport::new(port_name, 115200)
            .timeout(Duration::from_secs(1))
            .flow_control(serialport::FlowControl::Hardware)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .data_bits(serialport::DataBits::Eight)
            .open()
            .with_context(|| format!("Failed to open serial port {}", port_name))?;

        info!("Serial port {} opened successfully", port_name);
        Ok(Self {
            port,
            port_name: port_name.to_string(),
            reconnected: false,
            was_present: true,
        })
    }

    /// Auto-detect COM port by USB VID:PID and optional serial number filter
    pub fn auto_detect(vid_pid_pairs: &[(u16, u16)], serial_numbers: &[&str]) -> Result<String> {
        let ports = serialport::available_ports().context("Failed to enumerate serial ports")?;

        // First pass: match by serial number (most specific)
        for port_info in &ports {
            if let SerialPortType::UsbPort(usb) = &port_info.port_type {
                if let Some(ref sn) = usb.serial_number {
                    for expected_sn in serial_numbers {
                        if sn == *expected_sn {
                            debug!(
                                "Auto-detected port {} by serial number '{}'",
                                port_info.port_name, sn
                            );
                            return Ok(port_info.port_name.clone());
                        }
                    }
                }
            }
        }

        // Second pass: match by VID:PID
        for port_info in &ports {
            if let SerialPortType::UsbPort(usb) = &port_info.port_type {
                for (vid, pid) in vid_pid_pairs {
                    if usb.vid == *vid && usb.pid == *pid {
                        debug!(
                            "Auto-detected port {} by VID:PID {:04x}:{:04x}",
                            port_info.port_name, vid, pid
                        );
                        return Ok(port_info.port_name.clone());
                    }
                }
            }
        }

        Err(anyhow!(
            "No matching serial port found (checked {} ports)",
            ports.len()
        ))
    }

    /// List all available serial ports (for settings UI)
    #[allow(dead_code)]
    pub fn list_ports() -> Result<Vec<SerialPortInfo>> {
        serialport::available_ports().context("Failed to list serial ports")
    }

    /// Check if the COM port is currently present in the system.
    /// Detects USB cable disconnect/reconnect by polling the OS port list.
    /// Returns true if the port disappeared and came back (needs re-init).
    pub fn check_port_health(&mut self) -> bool {
        let is_present = serialport::available_ports()
            .map(|ports| ports.iter().any(|p| p.port_name == self.port_name))
            .unwrap_or(false);

        if !is_present && self.was_present {
            warn!(
                "Serial port {} disappeared — USB cable likely disconnected",
                self.port_name
            );
            self.was_present = false;
        } else if is_present && !self.was_present {
            info!(
                "Serial port {} reappeared — USB cable reconnected",
                self.port_name
            );
            self.was_present = true;
            // Reopen with a fresh handle since the old one is stale
            match Self::open_port(&self.port_name) {
                Ok(new_port) => {
                    self.port = new_port;
                    self.reconnected = true;
                    return true;
                }
                Err(e) => {
                    warn!("Failed to reopen {} after reconnect: {}", self.port_name, e);
                }
            }
        }

        false
    }

    /// Open a raw serial port handle (helper for reconnection)
    fn open_port(port_name: &str) -> Result<Box<dyn SerialPort>> {
        serialport::new(port_name, 115200)
            .timeout(Duration::from_secs(1))
            .flow_control(serialport::FlowControl::Hardware)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .data_bits(serialport::DataBits::Eight)
            .open()
            .with_context(|| format!("Failed to open serial port {}", port_name))
    }

    /// Write data to the serial port with error recovery.
    /// On failure, waits for the port to reconnect but does NOT retry the write.
    /// This prevents sending stale image data to a power-cycled display whose
    /// protocol parser would misinterpret raw pixel bytes as commands.
    /// The `reconnected` flag is set so the caller can re-initialize.
    pub fn write_data(&mut self, data: &[u8]) -> Result<()> {
        match self.port.write_all(data) {
            Ok(()) => Ok(()),
            Err(e) => {
                error!(
                    "Serial write failed on {}: {}. Attempting reconnect...",
                    self.port_name, e
                );
                self.reconnect_with_retry()?;
                Err(anyhow!(
                    "Serial port reconnected — caller should re-initialize before sending data"
                ))
            }
        }
    }

    /// Read exact number of bytes from the serial port
    pub fn read_data(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        match self.port.read_exact(&mut buf) {
            Ok(()) => Ok(buf),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                warn!(
                    "Serial read timed out on {} (expected {} bytes)",
                    self.port_name, size
                );
                Ok(Vec::new())
            }
            Err(e) => {
                error!(
                    "Serial read failed on {}: {}. Attempting reconnect...",
                    self.port_name, e
                );
                self.reconnect_with_retry()?;
                Err(anyhow!(
                    "Serial port reconnected — caller should re-initialize before sending data"
                ))
            }
        }
    }

    /// Flush the input buffer
    pub fn flush_input(&mut self) -> Result<()> {
        self.port
            .clear(serialport::ClearBuffer::Input)
            .context("Failed to flush serial input buffer")
    }

    /// Write data in chunks, yielding between chunks
    #[allow(dead_code)]
    pub fn write_chunked(&mut self, data: &[u8], chunk_size: usize) -> Result<()> {
        for chunk in data.chunks(chunk_size) {
            self.write_data(chunk)?;
        }
        Ok(())
    }

    /// Keep trying to reopen the serial port until successful or max retries exceeded.
    /// This handles USB cable disconnections — the port won't be available until
    /// the cable is plugged back in, so we retry every 2 seconds.
    fn reconnect_with_retry(&mut self) -> Result<()> {
        const MAX_RECONNECT_ATTEMPTS: u32 = 150; // ~5 minutes
        warn!(
            "Attempting to reconnect to serial port {}...",
            self.port_name
        );
        for attempt in 0..MAX_RECONNECT_ATTEMPTS {
            if SHUTDOWN.load(Ordering::Relaxed) {
                info!("Reconnect aborted: shutdown signalled");
                return Err(anyhow!("Shutdown"));
            }
            std::thread::sleep(Duration::from_secs(2));

            match Self::open_port(&self.port_name) {
                Ok(new_port) => {
                    self.port = new_port;
                    self.reconnected = true;
                    self.was_present = true;
                    info!("Reconnected to serial port {}", self.port_name);
                    return Ok(());
                }
                Err(e) => {
                    debug!(
                        "Reconnect attempt {}/{} to {} failed: {}. Retrying in 2s...",
                        attempt + 1,
                        MAX_RECONNECT_ATTEMPTS,
                        self.port_name,
                        e
                    );
                }
            }
        }
        Err(anyhow::anyhow!(
            "Failed to reconnect to {} after {} attempts",
            self.port_name,
            MAX_RECONNECT_ATTEMPTS
        ))
    }

    /// Check if the serial connection was recently reconnected (e.g. after cable unplug/replug).
    /// Returns true once after each reconnection, then resets the flag.
    pub fn take_reconnected(&mut self) -> bool {
        let was = self.reconnected;
        self.reconnected = false;
        was
    }

    /// Get the port name
    #[allow(dead_code)]
    pub fn port_name(&self) -> &str {
        &self.port_name
    }
}
