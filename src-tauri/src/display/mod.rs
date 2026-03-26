pub mod diff;
pub mod protocol_a;
pub mod protocol_b;
pub mod protocol_c;
pub mod protocol_d;
pub mod protocol_weact;
pub mod rgb565;
pub mod serial;

use anyhow::{Result, anyhow};
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
    fn display_rgba_image(
        &mut self,
        rgba: &[u8],
        x: u16,
        y: u16,
        w: u16,
        h: u16,
    ) -> Result<()>;

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
    let com_port = if config.com_port.is_empty() { "AUTO" } else { &config.com_port };
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
            let display = protocol_weact::WeActDisplay::new(
                com_port,
                protocol_weact::WeActVariant::A,
            )?;
            Ok(Box::new(display))
        }
        "WEACT_B" => {
            let display = protocol_weact::WeActDisplay::new(
                com_port,
                protocol_weact::WeActVariant::B,
            )?;
            Ok(Box::new(display))
        }
        _ => Err(anyhow!("Unknown display revision: '{}'", revision)),
    }
}
