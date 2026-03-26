/// Rev B protocol implementation for XuanFang 3.5" (standard & flagship) displays.
///
/// Ports `library/lcd/lcd_comm_rev_b.py` from the Python implementation.
///
/// Key differences from Rev A:
/// - 10-byte framed packets: [CMD, 8 data bytes, CMD]
/// - RGB565 big-endian (not little-endian)
/// - Reverse orientations handled in software (180° rotation)
/// - 50ms cooldown between bitmaps
/// - Flagship variant supports RGB backplate LED

use anyhow::{Context, Result};
use log::{info, warn};

use super::rgb565::{chunked, rgba_to_rgb565_be};
use super::serial::SerialConnection;
use super::{LcdDisplay, Orientation};

/// Rev B command set
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)] // Full command set for protocol completeness
enum Command {
    Hello = 0xCA,
    SetOrientation = 0xCB,
    DisplayBitmap = 0xCC,
    SetLighting = 0xCD,
    SetBrightness = 0xCE,
}

/// Hardware orientation values sent to the display
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
enum HwOrientation {
    Portrait = 0x00,
    Landscape = 0x01,
}

/// Sub-revision detected from HELLO response bytes 6-7
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevBSubRevision {
    A01, // Brightness binary (0/1), standard
    A02, // Brightness binary (0/1), flagship (RGB LED)
    A11, // Brightness 0-255, standard
    A12, // Brightness 0-255, flagship (RGB LED)
}

pub struct RevBDisplay {
    serial: SerialConnection,
    orientation: Orientation,
    display_width: u16,
    display_height: u16,
    sub_revision: RevBSubRevision,
}

impl RevBDisplay {
    const VID_PID: [(u16, u16); 1] = [(0x1a86, 0x5722)];
    const SERIAL_NUMBERS: [&str; 1] = ["2017-2-25"];

    pub fn new(com_port: &str) -> Result<Self> {
        let port_name = if com_port == "AUTO" {
            SerialConnection::auto_detect(&Self::VID_PID, &Self::SERIAL_NUMBERS)
                .context("Rev B auto-detect failed")?
        } else {
            com_port.to_string()
        };

        let serial = SerialConnection::open(&port_name)?;
        info!("Rev B display connected on {}", port_name);

        Ok(Self {
            serial,
            orientation: Orientation::Portrait,
            display_width: 320,
            display_height: 480,
            sub_revision: RevBSubRevision::A01,
        })
    }

    /// Build a 10-byte framed command packet: [CMD, payload[0..8], CMD]
    fn build_packet(cmd: Command, payload: &[u8]) -> [u8; 10] {
        let mut packet = [0u8; 10];
        packet[0] = cmd as u8;
        let copy_len = payload.len().min(8);
        packet[1..1 + copy_len].copy_from_slice(&payload[..copy_len]);
        packet[9] = cmd as u8;
        packet
    }

    fn send_command(&mut self, cmd: Command, payload: &[u8]) -> Result<()> {
        let packet = Self::build_packet(cmd, payload);
        self.serial.write_data(&packet)
    }

    /// HELLO handshake — detects sub-revision from response bytes 6-7
    fn hello(&mut self) -> Result<()> {
        let hello_payload = [b'H', b'E', b'L', b'L', b'O'];
        self.send_command(Command::Hello, &hello_payload)?;

        let response = self.serial.read_data(10)?;
        self.serial.flush_input()?;

        if response.len() != 10 {
            warn!("Rev B: short HELLO response ({} bytes)", response.len());
            return Ok(());
        }

        if response[0] != Command::Hello as u8 || response[9] != Command::Hello as u8 {
            warn!("Rev B: bad HELLO framing");
        }

        // Parse sub-revision from bytes 6-7
        match (response[6], response[7]) {
            (0x0A, 0x01) => self.sub_revision = RevBSubRevision::A01,
            (0x0A, 0x02) => self.sub_revision = RevBSubRevision::A02,
            (0x0A, 0x11) => self.sub_revision = RevBSubRevision::A11,
            (0x0A, 0x12) => self.sub_revision = RevBSubRevision::A12,
            _ => warn!(
                "Rev B: unknown sub-revision bytes [{:#04x}, {:#04x}]",
                response[6], response[7]
            ),
        }

        info!("Rev B sub-revision: {:?}", self.sub_revision);
        Ok(())
    }

    #[allow(dead_code)]
    fn is_flagship(&self) -> bool {
        matches!(
            self.sub_revision,
            RevBSubRevision::A02 | RevBSubRevision::A12
        )
    }

    fn is_brightness_range(&self) -> bool {
        matches!(
            self.sub_revision,
            RevBSubRevision::A11 | RevBSubRevision::A12
        )
    }

    /// Set backplate RGB LED color (flagship only)
    #[allow(dead_code)]
    pub fn set_led_color(&mut self, r: u8, g: u8, b: u8) -> Result<()> {
        if self.is_flagship() {
            self.send_command(Command::SetLighting, &[r, g, b])
        } else {
            info!("Only flagship revision supports backplate LED color");
            Ok(())
        }
    }

    /// Software-rotate RGBA image 180° for reverse orientations.
    /// Returns new buffer with rows and pixels reversed.
    fn rotate_180(rgba: &[u8], w: u16, h: u16) -> Vec<u8> {
        let stride = (w as usize) * 4;
        let mut rotated = Vec::with_capacity(rgba.len());
        for row in (0..h as usize).rev() {
            let row_start = row * stride;
            for col in (0..w as usize).rev() {
                let px = row_start + col * 4;
                if px + 4 <= rgba.len() {
                    rotated.extend_from_slice(&rgba[px..px + 4]);
                }
            }
        }
        rotated
    }
}

impl LcdDisplay for RevBDisplay {
    fn initialize(&mut self) -> Result<()> {
        self.hello()
    }

    fn reset(&mut self) -> Result<()> {
        // Rev B has no reset command — clear instead
        self.clear()
    }

    fn clear(&mut self) -> Result<()> {
        // No native clear: send a white full-screen image
        let saved = self.orientation;
        self.set_orientation(Orientation::Portrait)?;

        let w = self.get_width();
        let h = self.get_height();
        let white_rgba = vec![255u8; (w as usize) * (h as usize) * 4];
        self.display_rgba_image(&white_rgba, 0, 0, w, h)?;

        self.set_orientation(saved)?;
        Ok(())
    }

    fn screen_off(&mut self) -> Result<()> {
        self.set_brightness(0)
    }

    fn screen_on(&mut self) -> Result<()> {
        self.set_brightness(25)
    }

    fn set_brightness(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        let converted = if self.is_brightness_range() {
            ((level as u16 * 255) / 100) as u8
        } else {
            // Binary: 1 = off, 0 = full brightness
            if level == 0 { 1 } else { 0 }
        };
        self.send_command(Command::SetBrightness, &[converted])
    }

    fn set_orientation(&mut self, orientation: Orientation) -> Result<()> {
        self.orientation = orientation;
        let hw = match orientation {
            Orientation::Portrait | Orientation::ReversePortrait => HwOrientation::Portrait,
            Orientation::Landscape | Orientation::ReverseLandscape => HwOrientation::Landscape,
        };
        self.send_command(Command::SetOrientation, &[hw as u8])
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
        let display_h = self.get_height();

        let actual_w = w.min(display_w.saturating_sub(x));
        let actual_h = h.min(display_h.saturating_sub(y));

        // For reverse orientations, rotate image 180° in software and flip coordinates
        let (rgba_data, x0, y0, x1, y1) = match self.orientation {
            Orientation::Portrait | Orientation::Landscape => {
                (rgba.to_vec(), x, y, x + actual_w - 1, y + actual_h - 1)
            }
            Orientation::ReversePortrait | Orientation::ReverseLandscape => {
                let rotated = Self::rotate_180(rgba, actual_w, actual_h);
                let rx = display_w - x - actual_w;
                let ry = display_h - y - actual_h;
                (rotated, rx, ry, rx + actual_w - 1, ry + actual_h - 1)
            }
        };

        // Rev B bitmap command payload: coordinates as big-endian u16 pairs
        let payload = [
            (x0 >> 8) as u8,
            (x0 & 0xFF) as u8,
            (y0 >> 8) as u8,
            (y0 & 0xFF) as u8,
            (x1 >> 8) as u8,
            (x1 & 0xFF) as u8,
            (y1 >> 8) as u8,
            (y1 & 0xFF) as u8,
        ];
        self.send_command(Command::DisplayBitmap, &payload)?;

        // Convert to RGB565 big-endian (Rev B format)
        let rgb565 = rgba_to_rgb565_be(&rgba_data);

        // Send in chunks of display_width * 8 bytes
        let chunk_size = (display_w as usize) * 8;
        for chunk in chunked(&rgb565, chunk_size) {
            self.serial.write_data(chunk)?;
        }

        // 50ms cooldown between bitmaps to prevent corruption
        std::thread::sleep(std::time::Duration::from_millis(50));

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
    fn test_build_packet() {
        let packet = RevBDisplay::build_packet(Command::Hello, b"HELLO");
        assert_eq!(packet[0], 0xCA);
        assert_eq!(packet[9], 0xCA);
        assert_eq!(&packet[1..6], b"HELLO");
        assert_eq!(&packet[6..9], &[0, 0, 0]); // padding
    }

    #[test]
    fn test_build_packet_short_payload() {
        let packet = RevBDisplay::build_packet(Command::SetBrightness, &[128]);
        assert_eq!(packet[0], 0xCE);
        assert_eq!(packet[1], 128);
        assert_eq!(&packet[2..9], &[0; 7]); // rest is zero-padded
        assert_eq!(packet[9], 0xCE);
    }

    #[test]
    fn test_rotate_180() {
        // 2x2 RGBA image
        let rgba = [
            1, 0, 0, 255, // pixel (0,0) = red
            0, 1, 0, 255, // pixel (1,0) = green
            0, 0, 1, 255, // pixel (0,1) = blue
            1, 1, 1, 255, // pixel (1,1) = white
        ];
        let rotated = RevBDisplay::rotate_180(&rgba, 2, 2);
        // After 180° rotation: (1,1) becomes (0,0), etc.
        assert_eq!(&rotated[0..4], &[1, 1, 1, 255]); // white → top-left
        assert_eq!(&rotated[4..8], &[0, 0, 1, 255]); // blue → top-right
        assert_eq!(&rotated[8..12], &[0, 1, 0, 255]); // green → bottom-left
        assert_eq!(&rotated[12..16], &[1, 0, 0, 255]); // red → bottom-right
    }
}
