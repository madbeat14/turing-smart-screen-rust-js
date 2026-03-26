/// WeAct Studio display protocol implementation.
///
/// Ports `library/lcd/lcd_comm_weact_a.py` and `lcd_comm_weact_b.py` from the Python implementation.
///
/// Key characteristics:
/// - Simple command format: [CMD, payload..., CMD_END(0x0A)]
/// - RGB565 little-endian format
/// - Bitmap: 10-byte header [0x05, x0_lo, x0_hi, y0_lo, y0_hi, x1_lo, x1_hi, y1_lo, y1_hi, 0x0A]
/// - Brightness: 0-255 scale with 1000ms transition time
/// - Orientation sent as raw enum value
/// - Two variants:
///   - WeAct A: 3.5" (320x480), serial number starts with "AB"
///   - WeAct B: 0.96" (80x160), serial number starts with "AD"

use anyhow::{Context, Result};
use log::info;

use super::rgb565::{chunked, rgba_to_rgb565_le};
use super::serial::SerialConnection;
use super::{LcdDisplay, Orientation};

/// WeAct command bytes
const CMD_SET_ORIENTATION: u8 = 0x02;
const CMD_SET_BRIGHTNESS: u8 = 0x03;
const CMD_FULL: u8 = 0x04;
const CMD_SET_BITMAP: u8 = 0x05;
#[allow(dead_code)] // Part of protocol command set
const CMD_FREE: u8 = 0x07;
const CMD_END: u8 = 0x0A;
const CMD_SYSTEM_VERSION_READ: u8 = 0x42 | 0x80; // CMD_SYSTEM_VERSION | CMD_READ

/// WeAct display variant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeActVariant {
    /// WeAct A: FS V1 3.5" (320x480)
    A,
    /// WeAct B: FS V1 0.96" (80x160)
    B,
}

pub struct WeActDisplay {
    serial: SerialConnection,
    variant: WeActVariant,
    orientation: Orientation,
    display_width: u16,
    display_height: u16,
    brightness: u8,
}

impl WeActDisplay {
    const VID_PID: [(u16, u16); 1] = [(0x1a86, 0xfe0c)];

    pub fn new(com_port: &str, variant: WeActVariant) -> Result<Self> {
        let serial_prefixes: &[&str] = match variant {
            WeActVariant::A => &["AB"],
            WeActVariant::B => &["AD"],
        };

        let port_name = if com_port == "AUTO" {
            SerialConnection::auto_detect(&Self::VID_PID, serial_prefixes)
                .with_context(|| format!("WeAct {:?} auto-detect failed", variant))?
        } else {
            com_port.to_string()
        };

        let serial = SerialConnection::open(&port_name)?;

        let (w, h) = match variant {
            WeActVariant::A => (320, 480),
            WeActVariant::B => (80, 160),
        };

        info!("WeAct {:?} display connected on {}", variant, port_name);

        Ok(Self {
            serial,
            variant,
            orientation: Orientation::Portrait,
            display_width: w,
            display_height: h,
            brightness: 0,
        })
    }

    /// Send a command buffer
    fn send_command(&mut self, data: &[u8]) -> Result<()> {
        self.serial.write_data(data)?;
        Ok(())
    }

    /// Fill screen with a solid RGB565 color
    fn fill_color(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        let r5 = (r >> 3) as u16;
        let g6 = (g >> 2) as u16;
        let b5 = (b >> 3) as u16;
        let rgb565 = (r5 << 11) | (g6 << 5) | b5;
        let color_bytes = rgb565.to_le_bytes();

        let xe = self.get_width();
        let ye = self.get_height();

        let cmd = [
            CMD_FULL,
            0, 0, // x0 = 0
            0, 0, // y0 = 0
            ((xe - 1) & 0xFF) as u8,
            ((xe - 1) >> 8) as u8,
            ((ye - 1) & 0xFF) as u8,
            ((ye - 1) >> 8) as u8,
            color_bytes[0],
            color_bytes[1],
            CMD_END,
        ];
        self.send_command(&cmd)
    }

    /// Send the "free" command (release display resources)
    #[allow(dead_code)]
    fn free(&mut self) -> Result<()> {
        self.send_command(&[CMD_FREE, CMD_END])
    }
}

impl LcdDisplay for WeActDisplay {
    fn initialize(&mut self) -> Result<()> {
        // Flush any pending data
        self.serial.flush_input()?;

        // Query system version
        self.serial.write_data(&[CMD_SYSTEM_VERSION_READ, CMD_END])?;
        match self.serial.read_data(19) {
            Ok(response) if response.len() == 19 => {
                let version = String::from_utf8_lossy(&response[1..9]);
                info!("WeAct {:?} device version: {}", self.variant, version.trim());
            }
            _ => {
                info!("WeAct {:?}: could not read device version", self.variant);
            }
        }
        self.serial.flush_input()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // WeAct has no reset command
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        self.fill_color(0, 0, 0)
    }

    fn screen_off(&mut self) -> Result<()> {
        self.set_brightness(0)?;
        self.free()
    }

    fn screen_on(&mut self) -> Result<()> {
        self.set_brightness(self.brightness)
    }

    fn set_brightness(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        self.brightness = level;

        let converted = ((level as u16) * 255 / 100) as u8;
        let transition_ms: u16 = 1000;

        let cmd = [
            CMD_SET_BRIGHTNESS,
            converted,
            (transition_ms & 0xFF) as u8,
            (transition_ms >> 8) as u8,
            CMD_END,
        ];
        self.send_command(&cmd)
    }

    fn set_orientation(&mut self, orientation: Orientation) -> Result<()> {
        self.orientation = orientation;

        let cmd = [
            CMD_SET_ORIENTATION,
            orientation as u8,
            CMD_END,
        ];
        self.send_command(&cmd)
    }

    fn display_rgba_image(
        &mut self,
        rgba: &[u8],
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> Result<()> {
        let display_w = self.get_width();
        let actual_w = w.min(display_w.saturating_sub(x));
        let actual_h = h.min(self.get_height().saturating_sub(y));

        if actual_w == 0 || actual_h == 0 {
            return Ok(());
        }

        let x1 = x + actual_w - 1;
        let y1 = y + actual_h - 1;

        // Build 10-byte bitmap header: [CMD, x0_lo, x0_hi, y0_lo, y0_hi, x1_lo, x1_hi, y1_lo, y1_hi, END]
        let header = [
            CMD_SET_BITMAP,
            (x & 0xFF) as u8,
            (x >> 8) as u8,
            (y & 0xFF) as u8,
            (y >> 8) as u8,
            (x1 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (y1 & 0xFF) as u8,
            (y1 >> 8) as u8,
            CMD_END,
        ];
        self.serial.write_data(&header)?;

        // Convert to RGB565 little-endian
        let rgb565 = rgba_to_rgb565_le(rgba);

        // Send in chunks of display_width * 4 bytes
        let chunk_size = (display_w as usize) * 4;
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
    fn test_rgb565_fill_color() {
        // White: R=0xFF, G=0xFF, B=0xFF → R5=31, G6=63, B5=31 → 0xFFFF
        let r5 = (0xFFu8 >> 3) as u16; // 31
        let g6 = (0xFFu8 >> 2) as u16; // 63
        let b5 = (0xFFu8 >> 3) as u16; // 31
        let rgb565 = (r5 << 11) | (g6 << 5) | b5;
        assert_eq!(rgb565, 0xFFFF);
    }

    #[test]
    fn test_brightness_conversion() {
        // 100% → 255
        assert_eq!((100u16 * 255 / 100) as u8, 255);
        // 50% → 127
        assert_eq!((50u16 * 255 / 100) as u8, 127);
        // 0% → 0
        assert_eq!((0u16 * 255 / 100) as u8, 0);
    }

    #[test]
    fn test_bitmap_header_format() {
        // Verify the 10-byte header for a region at (10, 20) to (109, 119)
        let x: u16 = 10;
        let y: u16 = 20;
        let x1: u16 = 109;
        let y1: u16 = 119;

        let header = [
            CMD_SET_BITMAP,
            (x & 0xFF) as u8,
            (x >> 8) as u8,
            (y & 0xFF) as u8,
            (y >> 8) as u8,
            (x1 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (y1 & 0xFF) as u8,
            (y1 >> 8) as u8,
            CMD_END,
        ];

        assert_eq!(header[0], 0x05); // CMD_SET_BITMAP
        assert_eq!(header[1], 10);   // x lo
        assert_eq!(header[2], 0);    // x hi
        assert_eq!(header[3], 20);   // y lo
        assert_eq!(header[4], 0);    // y hi
        assert_eq!(header[5], 109);  // x1 lo
        assert_eq!(header[6], 0);    // x1 hi
        assert_eq!(header[7], 119);  // y1 lo
        assert_eq!(header[8], 0);    // y1 hi
        assert_eq!(header[9], 0x0A); // CMD_END
    }
}
