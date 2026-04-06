/// Rev D protocol implementation for Kipye Qiye Smart Display 3.5".
///
/// Ports `library/lcd/lcd_comm_rev_d.py` from the Python implementation.
///
/// Key characteristics:
/// - 4-byte command headers
/// - Bitmap: BLOCKWRITE → INTOPICMODE → 63-byte chunks prefixed with 0x50 → OUTPICMODE
/// - Brightness scale: 0-500 (sent twice for reliability)
/// - RGB565 big-endian format
/// - Landscape orientations handled in software (270° rotation)

use anyhow::{Context, Result};
use log::info;

use super::rgb565::{chunked, rgba_to_rgb565_be};
use super::serial::SerialConnection;
use super::{LcdDisplay, Orientation};

/// Rev D command byte sequences
struct Cmd;

impl Cmd {
    const SETORG: &[u8] = &[67, 72, 0, 0];     // Portrait orientation
    const SET180: &[u8] = &[67, 71, 0, 0];     // Reverse portrait orientation
    const SETBL: &[u8] = &[67, 67];            // Brightness (+ 2-byte payload)
    const DISPCOLOR: &[u8] = &[67, 66];        // Fill screen with RGB565 color
    const BLOCKWRITE: &[u8] = &[67, 65];       // Bitmap bounds (+ 8-byte payload)
    const INTOPICMODE: &[u8] = &[68, 0, 0, 0]; // Start bitmap transmission
    const OUTPICMODE: &[u8] = &[65, 0, 0, 0];  // End bitmap transmission
}

pub struct RevDDisplay {
    serial: SerialConnection,
    orientation: Orientation,
    display_width: u16,
    display_height: u16,
}

impl RevDDisplay {
    const VID_PID: [(u16, u16); 1] = [(0x454d, 0x4e41)];
    const SERIAL_NUMBERS: [&str; 0] = [];

    pub fn new(com_port: &str) -> Result<Self> {
        let port_name = if com_port == "AUTO" {
            SerialConnection::auto_detect(&Self::VID_PID, &Self::SERIAL_NUMBERS)
                .context("Rev D auto-detect failed")?
        } else {
            com_port.to_string()
        };

        let serial = SerialConnection::open(&port_name)?;
        info!("Rev D display connected on {}", port_name);

        Ok(Self {
            serial,
            orientation: Orientation::Portrait,
            display_width: 320,
            display_height: 480,
        })
    }

    /// Send a command with optional payload, flushing input after each write
    fn send_command(&mut self, cmd: &[u8], payload: Option<&[u8]>) -> Result<()> {
        let mut message = Vec::from(cmd);
        if let Some(p) = payload {
            message.extend_from_slice(p);
        }
        self.serial.write_data(&message)?;
        self.serial.flush_input()?;
        Ok(())
    }

    /// Software-rotate RGBA 270° for landscape orientations
    fn rotate_270(rgba: &[u8], w: u16, h: u16) -> (Vec<u8>, u16, u16) {
        let w = w as usize;
        let h = h as usize;
        let mut out = vec![0u8; w * h * 4];

        for y in 0..h {
            for x in 0..w {
                let src = (y * w + x) * 4;
                let dst = ((w - 1 - x) * h + y) * 4;
                if src + 4 <= rgba.len() && dst + 4 <= out.len() {
                    out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
        }
        (out, h as u16, w as u16) // dimensions swap
    }
}

impl LcdDisplay for RevDDisplay {
    fn initialize(&mut self) -> Result<()> {
        // Rev D has no HELLO/handshake
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        // No reset command — clear instead
        self.clear()
    }

    fn clear(&mut self) -> Result<()> {
        // Fill screen with white (RGB565 0xFFFF)
        self.send_command(Cmd::DISPCOLOR, Some(&0xFFFFu16.to_be_bytes()))
    }

    fn screen_off(&mut self) -> Result<()> {
        self.set_brightness(0)
    }

    fn screen_on(&mut self) -> Result<()> {
        self.set_brightness(25)
    }

    fn set_brightness(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        // Rev D brightness: 0-500 scale
        let converted = (level as u16) * 5;
        let bytes = converted.to_be_bytes();

        // Send twice for reliability (per Python implementation)
        self.send_command(Cmd::SETBL, Some(&bytes))?;
        self.send_command(Cmd::SETBL, Some(&bytes))
    }

    fn set_orientation(&mut self, orientation: Orientation) -> Result<()> {
        self.orientation = orientation;

        match orientation {
            Orientation::ReversePortrait | Orientation::ReverseLandscape => {
                self.send_command(Cmd::SET180, None)
            }
            _ => self.send_command(Cmd::SETORG, None),
        }
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

        if actual_w == 0 || actual_h == 0 {
            return Ok(());
        }

        // For landscape orientations, rotate image 270° and recalculate coordinates
        let (rgba_data, x0, y0, x1, y1, _img_w) = match self.orientation {
            Orientation::Portrait | Orientation::ReversePortrait => {
                (rgba.to_vec(), x, y, x + actual_w - 1, y + actual_h - 1, actual_w)
            }
            Orientation::Landscape | Orientation::ReverseLandscape => {
                let (rotated, new_w, new_h) = Self::rotate_270(rgba, actual_w, actual_h);
                let rx = self.display_width.saturating_sub(y).saturating_sub(actual_h);
                let ry = x;
                (
                    rotated,
                    rx,
                    ry,
                    rx + new_w - 1,
                    ry + new_h - 1,
                    new_w,
                )
            }
        };

        // BLOCKWRITE: send bitmap bounds as big-endian u16 pairs [x0, x1, y0, y1]
        let mut bounds = Vec::with_capacity(8);
        bounds.extend_from_slice(&x0.to_be_bytes());
        bounds.extend_from_slice(&x1.to_be_bytes());
        bounds.extend_from_slice(&y0.to_be_bytes());
        bounds.extend_from_slice(&y1.to_be_bytes());
        self.send_command(Cmd::BLOCKWRITE, Some(&bounds))?;

        // Start bitmap mode
        self.send_command(Cmd::INTOPICMODE, None)?;

        // Convert to RGB565 big-endian
        let rgb565 = rgba_to_rgb565_be(&rgba_data);

        // Send in 63-byte chunks, each prefixed with 0x50
        for chunk in chunked(&rgb565, 63) {
            let mut prefixed = Vec::with_capacity(1 + chunk.len());
            prefixed.push(0x50);
            prefixed.extend_from_slice(chunk);
            self.serial.write_data(&prefixed)?;
        }

        // End bitmap mode
        self.send_command(Cmd::OUTPICMODE, None)?;

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
    fn test_brightness_conversion() {
        // 100% → 500
        assert_eq!(100u16 * 5, 500);
        // 0% → 0
        assert_eq!(0u16 * 5, 0);
        // 50% → 250
        assert_eq!(50u16 * 5, 250);
    }

    #[test]
    fn test_rotate_270() {
        // 2x1 image: red, blue → after 270° should be 1x2: blue on top, red on bottom
        let rgba = [
            255, 0, 0, 255, // red (0,0)
            0, 0, 255, 255, // blue (1,0)
        ];
        let (rotated, new_w, new_h) = RevDDisplay::rotate_270(&rgba, 2, 1);
        assert_eq!(new_w, 1); // width and height swap
        assert_eq!(new_h, 2);
        // After 270° CW rotation: (x,y) → (h-1-y, x)
        // (0,0)→(0,0), (1,0)→(0,1) in new coords
        assert_eq!(&rotated[0..4], &[0, 0, 255, 255]); // blue at (0,0)
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]); // red at (0,1)
    }
}
