use anyhow::{anyhow, Context, Result};
use log::{debug, error, info, warn};
use serialport::{SerialPort, SerialPortInfo, SerialPortType};
use std::io::{Read, Write};
use std::time::Duration;

/// Serial port wrapper with error recovery and auto-detection
pub struct SerialConnection {
    port: Box<dyn SerialPort>,
    port_name: String,
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
        })
    }

    /// Auto-detect COM port by USB VID:PID and optional serial number filter
    pub fn auto_detect(
        vid_pid_pairs: &[(u16, u16)],
        serial_numbers: &[&str],
    ) -> Result<String> {
        let ports = serialport::available_ports()
            .context("Failed to enumerate serial ports")?;

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
    pub fn list_ports() -> Result<Vec<SerialPortInfo>> {
        serialport::available_ports().context("Failed to list serial ports")
    }

    /// Write data to the serial port with error recovery
    pub fn write_data(&mut self, data: &[u8]) -> Result<()> {
        match self.port.write_all(data) {
            Ok(()) => Ok(()),
            Err(e) => {
                error!(
                    "Serial write failed on {}: {}. Attempting reconnect...",
                    self.port_name, e
                );
                self.reconnect()?;
                self.port
                    .write_all(data)
                    .context("Serial write failed after reconnect")
            }
        }
    }

    /// Read exact number of bytes from the serial port
    pub fn read_data(&mut self, size: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; size];
        match self.port.read_exact(&mut buf) {
            Ok(()) => Ok(buf),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                warn!("Serial read timed out on {} (expected {} bytes)", self.port_name, size);
                Ok(Vec::new())
            }
            Err(e) => {
                error!(
                    "Serial read failed on {}: {}. Attempting reconnect...",
                    self.port_name, e
                );
                self.reconnect()?;
                self.port
                    .read_exact(&mut buf)
                    .context("Serial read failed after reconnect")?;
                Ok(buf)
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
    pub fn write_chunked(&mut self, data: &[u8], chunk_size: usize) -> Result<()> {
        for chunk in data.chunks(chunk_size) {
            self.write_data(chunk)?;
        }
        Ok(())
    }

    /// Close and reopen the serial port (error recovery)
    fn reconnect(&mut self) -> Result<()> {
        warn!("Reconnecting to serial port {}...", self.port_name);
        std::thread::sleep(Duration::from_secs(1));

        let new_port = serialport::new(&self.port_name, 115200)
            .timeout(Duration::from_secs(1))
            .flow_control(serialport::FlowControl::Hardware)
            .stop_bits(serialport::StopBits::One)
            .parity(serialport::Parity::None)
            .data_bits(serialport::DataBits::Eight)
            .open()
            .with_context(|| format!("Failed to reconnect to {}", self.port_name))?;

        self.port = new_port;
        info!("Reconnected to serial port {}", self.port_name);
        Ok(())
    }

    /// Get the port name
    pub fn port_name(&self) -> &str {
        &self.port_name
    }
}
