pub mod diff;
pub mod protocol_a;
pub mod protocol_b;
pub mod protocol_c;
pub mod protocol_d;
pub mod protocol_weact;
pub mod rgb565;
pub mod serial;

use anyhow::{anyhow, Result};
use log::info;

use crate::config::DisplayConfig;

/// Display orientation matching the Python Orientation enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)] // All orientations exist for protocol completeness
pub enum Orientation {
    Portrait = 0,
    ReversePortrait = 1,
    Landscape = 2,
    ReverseLandscape = 3,
}

/// Sub-revision detected during handshake
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubRevision {
    // Rev A sub-revisions
    Turing3_5,
    UsbMonitor3_5,
    UsbMonitor5,
    UsbMonitor7,
}

/// Common trait for all display protocol implementations
#[allow(dead_code)] // Trait defines the full protocol API; not all methods are called yet
pub trait LcdDisplay: Send {
    /// Run the handshake / initialization sequence
    fn initialize(&mut self) -> Result<()>;

    /// Reset the display (may cause COM port to change)
    fn reset(&mut self) -> Result<()>;

    /// Clear the screen (white)
    fn clear(&mut self) -> Result<()>;

    /// Turn screen off
    fn screen_off(&mut self) -> Result<()>;

    /// Turn screen on
    fn screen_on(&mut self) -> Result<()>;

    /// Set brightness (0-100 percent)
    fn set_brightness(&mut self, level: u8) -> Result<()>;

    /// Set display orientation
    fn set_orientation(&mut self, orientation: Orientation) -> Result<()>;

    /// Display an RGBA image at the given position
    /// `rgba` is raw RGBA pixel data, `w` x `h` pixels
    fn display_rgba_image(&mut self, rgba: &[u8], x: u16, y: u16, w: u16, h: u16) -> Result<()>;

    /// Get current display width (accounting for orientation)
    fn get_width(&self) -> u16;

    /// Get current display height (accounting for orientation)
    fn get_height(&self) -> u16;

    /// Check if the underlying connection was recently reconnected (e.g. USB cable replug).
    /// Returns true once per reconnection event, then resets.
    fn take_reconnected(&mut self) -> bool;

    /// Actively check whether the USB port is still present in the system.
    /// Returns true if the port disappeared and came back (cable unplug/replug detected).
    /// The caller should re-initialize the display when this returns true.
    fn check_port_health(&mut self) -> bool;
}

use std::borrow::Cow;

/// Crop an RGBA image buffer if actual dimensions differ from source dimensions.
/// Returns a borrowed slice when no cropping is needed (actual_w == src_w && actual_h == src_h),
/// avoiding unnecessary heap allocations.
pub fn crop_rgba_if_needed(rgba: &[u8], src_w: u16, actual_w: u16, actual_h: u16) -> Cow<'_, [u8]> {
    let src_w = src_w as usize;
    let actual_w = actual_w as usize;
    let actual_h = actual_h as usize;

    if actual_w * actual_h * 4 == rgba.len() && actual_w == src_w {
        return Cow::Borrowed(rgba);
    }

    let mut cropped = Vec::with_capacity(actual_w * actual_h * 4);
    let row_bytes = actual_w * 4;
    let stride = src_w * 4;

    for row in 0..actual_h {
        let start = row * stride;
        let end = start + row_bytes;
        if end <= rgba.len() {
            cropped.extend_from_slice(&rgba[start..end]);
        }
    }

    Cow::Owned(cropped)
}

/// Display dimensions for known hardware
pub fn dimensions_for_sub_revision(sub: SubRevision) -> (u16, u16) {
    match sub {
        SubRevision::Turing3_5 | SubRevision::UsbMonitor3_5 => (320, 480),
        SubRevision::UsbMonitor5 => (480, 800),
        SubRevision::UsbMonitor7 => (600, 1024),
    }
}

/// Factory function: create the right display driver based on config
pub fn create_display(config: &DisplayConfig) -> Result<Box<dyn LcdDisplay>> {
    let com_port = if config.com_port.is_empty() {
        "AUTO"
    } else {
        &config.com_port
    };
    let revision = config.revision.to_uppercase();

    info!("Creating display driver for revision '{}'", revision);

    match revision.as_str() {
        "A" => {
            let display = protocol_a::RevADisplay::new(com_port)?;
            Ok(Box::new(display))
        }
        "B" => {
            let display = protocol_b::RevBDisplay::new(com_port)?;
            Ok(Box::new(display))
        }
        "C" => {
            // Rev C needs explicit dimensions based on screen size
            let (w, h) = (480, 800); // Default to 5"; config should specify
            let display = protocol_c::RevCDisplay::new(com_port, w, h)?;
            Ok(Box::new(display))
        }
        "D" => {
            let display = protocol_d::RevDDisplay::new(com_port)?;
            Ok(Box::new(display))
        }
        "WEACT_A" => {
            let display =
                protocol_weact::WeActDisplay::new(com_port, protocol_weact::WeActVariant::A)?;
            Ok(Box::new(display))
        }
        "WEACT_B" => {
            let display =
                protocol_weact::WeActDisplay::new(com_port, protocol_weact::WeActVariant::B)?;
            Ok(Box::new(display))
        }
        _ => Err(anyhow!("Unknown display revision: '{}'", revision)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crop_rgba_if_needed_borrowed() {
        let rgba = vec![128u8; 10 * 10 * 4];
        let res = crop_rgba_if_needed(&rgba, 10, 10, 10);
        assert!(matches!(res, Cow::Borrowed(_)));
        assert_eq!(res.len(), 400);
    }

    #[test]
    fn test_crop_rgba_if_needed_cropped() {
        // 4x4 image, each row has a different pixel value
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for y in 0..4 {
            for x in 0..4 {
                let idx = (y * 4 + x) * 4;
                rgba[idx] = y as u8;
            }
        }
        // Crop to 2x2
        let res = crop_rgba_if_needed(&rgba, 4, 2, 2);
        assert!(matches!(res, Cow::Owned(_)));
        assert_eq!(res.len(), 2 * 2 * 4);
        // Row 0
        assert_eq!(res[0], 0);
        assert_eq!(res[4], 0);
        // Row 1
        assert_eq!(res[8], 1);
        assert_eq!(res[12], 1);
    }
}
