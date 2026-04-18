/// Rev A protocol implementation for Turing 3.5" and UsbMonitor 3.5"/5"/7" displays.
///
/// Ports `library/lcd/lcd_comm_rev_a.py` from the Python implementation.
use anyhow::{Context, Result};
use log::{debug, info};

use super::rgb565::{chunked, rgba_to_rgb565_le};
use super::serial::SerialConnection;
use super::{dimensions_for_sub_revision, LcdDisplay, Orientation, SubRevision};

/// Rev A command set
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)] // Full command set for protocol completeness
enum Command {
    Reset = 101,
    Clear = 102,
    ToBlack = 103,
    ScreenOff = 108,
    ScreenOn = 109,
    SetBrightness = 110,
    SetOrientation = 121,
    DisplayBitmap = 197,
    Hello = 69,
}

/// Rev A display driver
pub struct RevADisplay {
    serial: SerialConnection,
    orientation: Orientation,
    /// Display width in default orientation (portrait)
    display_width: u16,
    /// Display height in default orientation (portrait)
    display_height: u16,
    sub_revision: SubRevision,
}

impl RevADisplay {
    /// USB identifiers for auto-detection
    const VID_PID: [(u16, u16); 1] = [(0x1a86, 0x5722)];
    const SERIAL_NUMBERS: [&str; 1] = ["USB35INCHIPSV2"];

    /// Create a new Rev A display, opening the serial port
    pub fn new(com_port: &str) -> Result<Self> {
        let port_name = if com_port == "AUTO" {
            SerialConnection::auto_detect(&Self::VID_PID, &Self::SERIAL_NUMBERS)
                .context("Rev A auto-detect failed")?
        } else {
            com_port.to_string()
        };

        let serial = SerialConnection::open(&port_name)?;
        info!("Rev A display connected on {}", port_name);

        Ok(Self {
            serial,
            orientation: Orientation::Portrait,
            display_width: 320,
            display_height: 480,
            sub_revision: SubRevision::Turing3_5,
        })
    }

    /// Encode a 6-byte command packet.
    ///
    /// Packet format from Python:
    /// ```
    /// byte[0] = x >> 2
    /// byte[1] = ((x & 3) << 6) + (y >> 4)
    /// byte[2] = ((y & 15) << 4) + (ex >> 6)
    /// byte[3] = ((ex & 63) << 2) + (ey >> 8)
    /// byte[4] = ey & 255
    /// byte[5] = command
    /// ```
    fn encode_command(cmd: Command, x: u16, y: u16, ex: u16, ey: u16) -> [u8; 6] {
        [
            (x >> 2) as u8,
            (((x & 3) << 6) + (y >> 4)) as u8,
            (((y & 15) << 4) + (ex >> 6)) as u8,
            (((ex & 63) << 2) + (ey >> 8)) as u8,
            (ey & 255) as u8,
            cmd as u8,
        ]
    }

    /// Send a 6-byte command
    fn send_command(&mut self, cmd: Command, x: u16, y: u16, ex: u16, ey: u16) -> Result<()> {
        let packet = Self::encode_command(cmd, x, y, ex, ey);
        self.serial.write_data(&packet)
    }

    /// HELLO handshake to detect display model/sub-revision.
    ///
    /// Sends 6 bytes of 0x45 (HELLO), reads 6-byte response:
    /// - No response / timeout → Turing 3.5" (320x480)
    /// - [0x01 * 6] → UsbMonitor 3.5" (320x480)
    /// - [0x02 * 6] → UsbMonitor 5" (480x800)
    /// - [0x03 * 6] → UsbMonitor 7" (600x1024)
    fn hello(&mut self) -> Result<()> {
        let hello_packet = [Command::Hello as u8; 6];
        self.serial.write_data(&hello_packet)?;

        let response = self.serial.read_data(6)?;
        self.serial.flush_input()?;

        if response.is_empty() {
            self.sub_revision = SubRevision::Turing3_5;
        } else if response == [0x01; 6] {
            self.sub_revision = SubRevision::UsbMonitor3_5;
        } else if response == [0x02; 6] {
            self.sub_revision = SubRevision::UsbMonitor5;
        } else if response == [0x03; 6] {
            self.sub_revision = SubRevision::UsbMonitor7;
        } else {
            debug!(
                "Unknown HELLO response: {:?}, assuming Turing 3.5\"",
                response
            );
            self.sub_revision = SubRevision::Turing3_5;
        }

        let (w, h) = dimensions_for_sub_revision(self.sub_revision);
        self.display_width = w;
        self.display_height = h;

        info!("Rev A sub-revision: {:?} ({}x{})", self.sub_revision, w, h);
        Ok(())
    }
}

impl LcdDisplay for RevADisplay {
    fn initialize(&mut self) -> Result<()> {
        self.hello()
    }

    fn reset(&mut self) -> Result<()> {
        info!("Display reset (COM port may change)...");
        self.send_command(Command::Reset, 0, 0, 0, 0)?;
        // Display disconnects and reconnects after reset; port may change
        std::thread::sleep(std::time::Duration::from_secs(5));
        // Reconnect will happen on next serial operation via error recovery
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        // Bug workaround from Python: orientation needs to be PORTRAIT before clearing
        let saved = self.orientation;
        self.set_orientation(Orientation::Portrait)?;
        self.send_command(Command::Clear, 0, 0, 0, 0)?;
        self.set_orientation(saved)?;
        Ok(())
    }

    fn screen_off(&mut self) -> Result<()> {
        self.send_command(Command::ScreenOff, 0, 0, 0, 0)
    }

    fn screen_on(&mut self) -> Result<()> {
        self.send_command(Command::ScreenOn, 0, 0, 0, 0)
    }

    fn set_brightness(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        // Rev A brightness is inverted: 0 = brightest, 255 = darkest
        let level_absolute = 255 - ((level as u16 * 255) / 100);
        self.send_command(Command::SetBrightness, level_absolute, 0, 0, 0)
    }

    fn set_orientation(&mut self, orientation: Orientation) -> Result<()> {
        self.orientation = orientation;
        let width = self.get_width();
        let height = self.get_height();

        // Orientation uses a 16-byte extended packet
        let mut packet = [0u8; 16];
        // First 5 bytes are the standard command encoding with x=0,y=0,ex=0,ey=0
        packet[5] = Command::SetOrientation as u8;
        packet[6] = (orientation as u8) + 100;
        packet[7] = (width >> 8) as u8;
        packet[8] = (width & 0xFF) as u8;
        packet[9] = (height >> 8) as u8;
        packet[10] = (height & 0xFF) as u8;
        // bytes 11-15 are zero padding

        self.serial.write_data(&packet)
    }

    fn display_rgba_image(&mut self, rgba: &[u8], x: u16, y: u16, w: u16, h: u16) -> Result<()> {
        let display_w = self.get_width();
        let display_h = self.get_height();

        // Clamp image to display bounds (saturating_sub prevents u16 underflow)
        let actual_w = w.min(display_w.saturating_sub(x));
        let actual_h = h.min(display_h.saturating_sub(y));

        if actual_w == 0 || actual_h == 0 {
            return Ok(());
        }

        // Crop RGBA data if needed
        let rgba_data = if actual_w != w || actual_h != h {
            let mut cropped = Vec::with_capacity((actual_w as usize) * (actual_h as usize) * 4);
            for row in 0..actual_h as usize {
                let start = (row * w as usize) * 4;
                let end = start + (actual_w as usize) * 4;
                if end <= rgba.len() {
                    cropped.extend_from_slice(&rgba[start..end]);
                }
            }
            cropped
        } else {
            rgba.to_vec()
        };

        let (x0, y0) = (x, y);
        let (x1, y1) = (x + actual_w - 1, y + actual_h - 1);

        // Convert to RGB565 little-endian (Rev A format)
        let rgb565 = rgba_to_rgb565_le(&rgba_data);

        // Send DISPLAY_BITMAP command with coordinates
        self.send_command(Command::DisplayBitmap, x0, y0, x1, y1)?;

        // Send image data in chunks of display_width * 8 bytes
        let chunk_size = (display_w as usize) * 8;
        for chunk in chunked(&rgb565, chunk_size) {
            self.serial.write_data(chunk)?;
        }

        Ok(())
    }

    fn get_width(&self) -> u16 {
        match self.orientation {
            Orientation::Portrait | Orientation::ReversePortrait => self.display_width,
            Orientation::Landscape | Orientation::ReverseLandscape => self.display_height,
        }
    }

    fn get_height(&self) -> u16 {
        match self.orientation {
            Orientation::Portrait | Orientation::ReversePortrait => self.display_height,
            Orientation::Landscape | Orientation::ReverseLandscape => self.display_width,
        }
    }

    fn take_reconnected(&mut self) -> bool {
        self.serial.take_reconnected()
    }

    fn check_port_health(&mut self) -> bool {
        self.serial.check_port_health()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_command_basic() {
        let packet = RevADisplay::encode_command(Command::Clear, 0, 0, 0, 0);
        assert_eq!(packet[5], 102); // Command::Clear
        assert_eq!(packet[0..5], [0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_encode_command_with_coords() {
        // Test encoding x=100, y=200, ex=300, ey=400
        let packet = RevADisplay::encode_command(Command::DisplayBitmap, 100, 200, 300, 400);
        assert_eq!(packet[5], 197); // Command::DisplayBitmap

        // Verify coordinate encoding
        let x: u16 = 100;
        let y: u16 = 200;
        let ex: u16 = 300;
        let ey: u16 = 400;
        assert_eq!(packet[0], (x >> 2) as u8);
        assert_eq!(packet[1], (((x & 3) << 6) + (y >> 4)) as u8);
        assert_eq!(packet[2], (((y & 15) << 4) + (ex >> 6)) as u8);
        assert_eq!(packet[3], (((ex & 63) << 2) + (ey >> 8)) as u8);
        assert_eq!(packet[4], (ey & 255) as u8);
    }

    #[test]
    fn test_brightness_conversion() {
        // 100% brightness → level_absolute = 0 (brightest)
        let level: u8 = 100;
        let absolute = 255 - ((level as u16 * 255) / 100) as u16;
        assert_eq!(absolute, 0);

        // 0% brightness → level_absolute = 255 (darkest)
        let level: u8 = 0;
        let absolute = 255 - ((level as u16 * 255) / 100) as u16;
        assert_eq!(absolute, 255);

        // 50% brightness → ~127
        let level: u8 = 50;
        let absolute = 255 - ((level as u16 * 255) / 100) as u16;
        assert_eq!(absolute, 128); // 255 - 127 = 128
    }
}
