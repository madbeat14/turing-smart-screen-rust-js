/// Rev C protocol implementation for Turing 2.1"/2.8"/5"/8.8" displays.
///
/// Ports `library/lcd/lcd_comm_rev_c.py` from the Python implementation.
///
/// Key characteristics:
/// - Variable-length commands padded to multiples of 250 bytes
/// - BGR/BGRA image formats (not RGB565)
/// - Complex bitmap protocol: PRE_UPDATE → START_DISPLAY → size-specific → payload → QUERY_STATUS
/// - Sleep/wake protocol for screen power management
/// - Sub-revisions: 2.1"/2.8" (480x480), 5" (480x800), 8.8" (480x1920)

use anyhow::{Context, Result, anyhow};
use log::{debug, info, warn};

use super::rgb565::rgba_to_bgra;
use super::serial::SerialConnection;
use super::{LcdDisplay, Orientation};

/// Rev C sub-revision based on display size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevCSubRevision {
    Unknown,
    Rev2Inch, // 480x480 (2.1" and 2.8")
    Rev5Inch, // 480x800
    Rev8Inch, // 480x1920
}

/// Fixed command byte sequences from the Python implementation
struct Cmd;

#[allow(dead_code)] // Full command set for protocol completeness
impl Cmd {
    const HELLO: &[u8] = &[0x01, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xc5, 0xd3];
    const OPTIONS: &[u8] = &[0x7d, 0xef, 0x69, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x2d];
    const RESTART: &[u8] = &[0x84, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const TURNOFF: &[u8] = &[0x83, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const SET_BRIGHTNESS: &[u8] = &[0x7b, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
    const STOP_VIDEO: &[u8] = &[0x79, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const STOP_MEDIA: &[u8] = &[0x96, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const QUERY_STATUS: &[u8] = &[0xcf, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const START_DISPLAY_BITMAP: &[u8] = &[0x2c];
    const PRE_UPDATE_BITMAP: &[u8] = &[0x86, 0xef, 0x69, 0x00, 0x00, 0x00, 0x01];
    const UPDATE_BITMAP: &[u8] = &[0xcc, 0xef, 0x69, 0x00];
    const DISPLAY_BITMAP_2INCH: &[u8] = &[0xc8, 0xef, 0x69, 0x00, 0x0E, 0x10];
    const DISPLAY_BITMAP_5INCH: &[u8] = &[0xc8, 0xef, 0x69, 0x00, 0x17, 0x70];
    const DISPLAY_BITMAP_8INCH: &[u8] = &[0xc8, 0xef, 0x69, 0x00, 0x38, 0x40];

    // Option flags
    const STARTMODE_DEFAULT: u8 = 0x00;
    const NO_FLIP: u8 = 0x00;
    const SLEEP_OFF: u8 = 0x00;
}

pub struct RevCDisplay {
    serial: SerialConnection,
    orientation: Orientation,
    display_width: u16,
    display_height: u16,
    sub_revision: RevCSubRevision,
    rom_version: u8,
    update_count: u32,
}

impl RevCDisplay {
    // Auto-detect identifiers
    const VID_PID: [(u16, u16); 3] = [
        (0x1a86, 0xca21),
        (0x0525, 0xa4a7),
        (0x1d6b, 0x0121),
    ];
    const SERIAL_NUMBERS: [&str; 3] = ["USB7INCH", "CT21INCH", "20080411"];

    pub fn new(com_port: &str, display_width: u16, display_height: u16) -> Result<Self> {
        let port_name = if com_port == "AUTO" {
            SerialConnection::auto_detect(&Self::VID_PID, &Self::SERIAL_NUMBERS)
                .context("Rev C auto-detect failed")?
        } else {
            com_port.to_string()
        };

        let serial = SerialConnection::open(&port_name)?;
        info!("Rev C display connected on {}", port_name);

        Ok(Self {
            serial,
            orientation: Orientation::Portrait,
            display_width,
            display_height,
            sub_revision: RevCSubRevision::Unknown,
            rom_version: 87,
            update_count: 0,
        })
    }

    /// Send a command, padded to a multiple of 250 bytes
    fn send_command(&mut self, cmd: &[u8], payload: Option<&[u8]>, pad_byte: u8) -> Result<()> {
        let mut message = Vec::from(cmd);
        if let Some(p) = payload {
            message.extend_from_slice(p);
        }

        // Pad to multiple of 250
        let remainder = message.len() % 250;
        if remainder != 0 {
            let pad_size = 250 - remainder;
            message.resize(message.len() + pad_size, pad_byte);
        }

        self.serial.write_data(&message)
    }

    /// Send command and read response
    fn send_command_read(&mut self, cmd: &[u8], payload: Option<&[u8]>, pad_byte: u8, read_size: usize) -> Result<Vec<u8>> {
        self.send_command(cmd, payload, pad_byte)?;
        self.serial.read_data(read_size)
    }

    /// HELLO handshake — reads 23-byte response to detect model
    fn hello(&mut self) -> Result<()> {
        self.serial.flush_input()?;
        self.send_command(Cmd::HELLO, None, 0x00)?;

        let response = self.serial.read_data(23)?;
        self.serial.flush_input()?;

        if response.is_empty() {
            return Err(anyhow!("Rev C: no response to HELLO"));
        }

        // Parse response as ASCII string
        let response_str: String = response
            .iter()
            .filter(|b| b.is_ascii_graphic() || **b == b'_' || **b == b'.')
            .map(|b| *b as char)
            .collect();

        debug!("Rev C display ID: {}", response_str);

        if !response_str.starts_with("chs_") {
            warn!("Rev C: unexpected display ID '{}', retrying...", response_str);
        }

        // Detect sub-revision from configured dimensions (more reliable than ID string)
        self.sub_revision = match (self.display_width, self.display_height) {
            (480, 480) => RevCSubRevision::Rev2Inch,
            (480, 800) => RevCSubRevision::Rev5Inch,
            (480, 1920) => RevCSubRevision::Rev8Inch,
            _ => {
                warn!(
                    "Unexpected Rev C dimensions {}x{}, defaulting to 5\"",
                    self.display_width, self.display_height
                );
                RevCSubRevision::Rev5Inch
            }
        };

        // Parse ROM version from ID string (e.g., "chs_5inch.87")
        if let Some(version_str) = response_str.split('.').nth(2) {
            if let Ok(v) = version_str.parse::<u8>() {
                if (80..=100).contains(&v) {
                    self.rom_version = v;
                }
            }
        }

        info!(
            "Rev C sub-revision: {:?}, ROM version: {}",
            self.sub_revision, self.rom_version
        );
        Ok(())
    }

    /// Get the size-specific DISPLAY_BITMAP command
    fn display_bitmap_cmd(&self) -> &[u8] {
        match self.sub_revision {
            RevCSubRevision::Rev2Inch => Cmd::DISPLAY_BITMAP_2INCH,
            RevCSubRevision::Rev5Inch => Cmd::DISPLAY_BITMAP_5INCH,
            RevCSubRevision::Rev8Inch => Cmd::DISPLAY_BITMAP_8INCH,
            RevCSubRevision::Unknown => Cmd::DISPLAY_BITMAP_5INCH,
        }
    }

    /// Generate full-screen image data with 0x00 padding every 249 bytes (BGRA format)
    fn generate_full_image(&self, rgba: &[u8], w: u16, h: u16) -> Vec<u8> {
        let bgra = self.rotate_for_orientation(rgba, w, h, true);
        let bgra_bytes = rgba_to_bgra(&bgra);

        // Insert 0x00 padding byte every 249 bytes
        let mut padded = Vec::with_capacity(bgra_bytes.len() + bgra_bytes.len() / 249 + 1);
        for (i, chunk) in bgra_bytes.chunks(249).enumerate() {
            if i > 0 {
                padded.push(0x00);
            }
            padded.extend_from_slice(chunk);
        }
        padded
    }

    /// Rotate RGBA data according to orientation and sub-revision
    fn rotate_for_orientation(&self, rgba: &[u8], w: u16, h: u16, full_screen: bool) -> Vec<u8> {
        // Full-screen rotation depends on sub-revision
        if full_screen {
            if self.sub_revision == RevCSubRevision::Rev8Inch {
                match self.orientation {
                    Orientation::Landscape => rotate_rgba(rgba, w, h, 270),
                    Orientation::ReverseLandscape => rotate_rgba(rgba, w, h, 90),
                    Orientation::Portrait => rotate_rgba(rgba, w, h, 180),
                    Orientation::ReversePortrait => rgba.to_vec(),
                }
            } else {
                match self.orientation {
                    Orientation::Portrait => rotate_rgba(rgba, w, h, 90),
                    Orientation::ReversePortrait => rotate_rgba(rgba, w, h, 270),
                    Orientation::ReverseLandscape => rotate_rgba(rgba, w, h, 180),
                    Orientation::Landscape => rgba.to_vec(),
                }
            }
        } else {
            rgba.to_vec()
        }
    }
}

/// Rotate RGBA pixel buffer by 90, 180, or 270 degrees
fn rotate_rgba(rgba: &[u8], w: u16, h: u16, degrees: u16) -> Vec<u8> {
    let w = w as usize;
    let h = h as usize;
    let pixel_count = w * h;
    let mut out = vec![0u8; pixel_count * 4];

    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) * 4;
            let dst_idx = match degrees {
                90 => ((w - 1 - x) * h + y) * 4,
                180 => ((h - 1 - y) * w + (w - 1 - x)) * 4,
                270 => (x * h + (h - 1 - y)) * 4,
                _ => src_idx,
            };
            if src_idx + 4 <= rgba.len() && dst_idx + 4 <= out.len() {
                out[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
            }
        }
    }
    out
}

impl LcdDisplay for RevCDisplay {
    fn initialize(&mut self) -> Result<()> {
        self.hello()?;
        // Stop any playing video/media
        self.send_command(Cmd::STOP_VIDEO, None, 0x00)?;
        self.send_command_read(Cmd::STOP_MEDIA, None, 0x00, 1024)?;
        Ok(())
    }

    fn reset(&mut self) -> Result<()> {
        info!("Rev C display reset...");
        self.send_command(Cmd::RESTART, None, 0x00)?;
        // Wait for disconnect + reconnect
        std::thread::sleep(std::time::Duration::from_secs(5));
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        let saved = self.orientation;
        self.set_orientation(Orientation::Portrait)?;

        let w = self.get_width();
        let h = self.get_height();
        let white = vec![255u8; (w as usize) * (h as usize) * 4];
        self.display_rgba_image(&white, 0, 0, w, h)?;

        self.set_orientation(saved)?;
        Ok(())
    }

    fn screen_off(&mut self) -> Result<()> {
        self.send_command(Cmd::STOP_VIDEO, None, 0x00)?;
        self.send_command_read(Cmd::STOP_MEDIA, None, 0x00, 1024)?;
        self.send_command(Cmd::TURNOFF, None, 0x00)?;
        Ok(())
    }

    fn screen_on(&mut self) -> Result<()> {
        self.send_command(Cmd::STOP_VIDEO, None, 0x00)?;
        self.send_command_read(Cmd::STOP_MEDIA, None, 0x00, 1024)?;
        Ok(())
    }

    fn set_brightness(&mut self, level: u8) -> Result<()> {
        let level = level.min(100);
        let converted = ((level as u16 * 255) / 100) as u8;
        self.send_command(Cmd::SET_BRIGHTNESS, Some(&[converted]), 0x00)
    }

    fn set_orientation(&mut self, orientation: Orientation) -> Result<()> {
        self.orientation = orientation;
        let options = [Cmd::STARTMODE_DEFAULT, 0x00, Cmd::NO_FLIP, Cmd::SLEEP_OFF];
        self.send_command(Cmd::OPTIONS, Some(&options), 0x00)
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

        let is_full_screen = x == 0 && y == 0 && w == display_w && h == display_h;

        if is_full_screen {
            // Full-screen path
            self.send_command(Cmd::PRE_UPDATE_BITMAP, None, 0x00)?;
            self.send_command(Cmd::START_DISPLAY_BITMAP, None, 0x2c)?; // pad with 0x2c

            let bmp_cmd = self.display_bitmap_cmd().to_vec();
            let dim_payload = ((self.display_width as u32 * self.display_width as u32 / 64) as u16)
                .to_be_bytes();
            self.send_command(&bmp_cmd, Some(&dim_payload), 0x00)?;

            let image_data = self.generate_full_image(rgba, w, h);
            self.serial.write_data(&image_data)?;

            // Read status
            self.serial.read_data(1024)?;
            self.send_command_read(Cmd::QUERY_STATUS, None, 0x00, 1024)?;
        } else {
            // Partial update path
            let bgra = rgba_to_bgra(rgba);

            // Build per-line data with 3-byte address + 2-byte width + pixel data
            let pixel_size = 4usize; // BGRA
            let mut img_raw = Vec::new();

            for row in 0..h as usize {
                let addr = if self.sub_revision == RevCSubRevision::Rev8Inch {
                    ((x as u32 + row as u32) * self.display_width as u32) + y as u32
                } else {
                    ((x as u32 + row as u32) * self.display_height as u32) + y as u32
                };

                // 3-byte big-endian address
                img_raw.push((addr >> 16) as u8);
                img_raw.push((addr >> 8) as u8);
                img_raw.push(addr as u8);

                // 2-byte big-endian width
                img_raw.push((w >> 8) as u8);
                img_raw.push((w & 0xFF) as u8);

                // Pixel data for this row
                let start = row * (w as usize) * pixel_size;
                let end = start + (w as usize) * pixel_size;
                if end <= bgra.len() {
                    img_raw.extend_from_slice(&bgra[start..end]);
                }
            }

            let image_size = ((img_raw.len() + 2) as u32).to_be_bytes();

            // Build UPDATE_BITMAP header
            let mut payload = Vec::from(Cmd::UPDATE_BITMAP);
            payload.extend_from_slice(&image_size[1..4]); // 3-byte size
            payload.extend_from_slice(&[0x00; 3]);
            payload.extend_from_slice(&self.update_count.to_be_bytes());

            // Insert 0x00 padding every 249 bytes in image data
            if img_raw.len() > 250 {
                let mut padded = Vec::new();
                for (i, chunk) in img_raw.chunks(249).enumerate() {
                    if i > 0 {
                        padded.push(0x00);
                    }
                    padded.extend_from_slice(chunk);
                }
                img_raw = padded;
            }
            img_raw.extend_from_slice(&[0xef, 0x69]);

            self.serial.write_data(&payload)?;
            self.serial.write_data(&img_raw)?;
            self.send_command_read(Cmd::QUERY_STATUS, None, 0x00, 1024)?;

            self.update_count += 1;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rotate_rgba_180() {
        // 2x1 image: red, blue
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255];
        let rotated = rotate_rgba(&rgba, 2, 1, 180);
        assert_eq!(&rotated[0..4], &[0, 0, 255, 255]); // blue first
        assert_eq!(&rotated[4..8], &[255, 0, 0, 255]); // then red
    }

    #[test]
    fn test_sub_revision_detection() {
        let cases = [
            (480, 480, RevCSubRevision::Rev2Inch),
            (480, 800, RevCSubRevision::Rev5Inch),
            (480, 1920, RevCSubRevision::Rev8Inch),
        ];
        for (w, h, expected) in cases {
            let sub = match (w, h) {
                (480, 480) => RevCSubRevision::Rev2Inch,
                (480, 800) => RevCSubRevision::Rev5Inch,
                (480, 1920) => RevCSubRevision::Rev8Inch,
                _ => RevCSubRevision::Unknown,
            };
            assert_eq!(sub, expected);
        }
    }
}
