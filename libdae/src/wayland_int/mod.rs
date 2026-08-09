// TODO: Figure out naming of things (make things more consistent) and figure out what goes where (Like ScreenSpace and such).
//! The interface between the privileged process and wayland.

use serde::{Deserialize, Serialize};

/// Interfaces with the display outputs.
pub mod comp_client_interface;
pub mod shell_layers;
pub mod wayland_context;

pub type Pixel = i32;
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
/// Positional information for a screen.
pub struct ScreenInfo {
    /// Size of the screen.
    pub size: Size,
    /// Position of the origin relative ot the other monitors.
    pub origin: Point,
}

/// Span of the output monitors.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Range {
    /// Top left corner pixel of the screen array.
    pub top_left: Point,
    /// Bottom right corner pixel of the screen array.
    pub bottom_right: Point,
}
/// Information about a monitor/screen layout.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ScreenSpace {
    /// The raw pixel range of the monitor layout.
    /// (top_left_corner, bottom_right_corner)
    range: Range,
    /// List of monitors sorted top to bottom, left to right.
    monitors: Vec<ScreenInfo>,
}
impl ScreenSpace {
    pub fn from_monitors(monitors: &[ScreenInfo]) -> Self {
        let mut range = Range {
            top_left: Default::default(),
            bottom_right: Default::default(),
        };
        for monitor in monitors {
            let o = monitor.origin;
            range.top_left.x = range.top_left.x.min(o.x);
            range.top_left.y = range.top_left.y.min(o.y);
            range.bottom_right.x = range.bottom_right.x.max(o.x + monitor.size.width as i32);
            range.bottom_right.y = range.bottom_right.y.max(o.y + monitor.size.height as i32);
        }
        // If the screen has width W, then the last pixel is at position W - 1.
        range.bottom_right.x -= 1;
        range.bottom_right.y -= 1;

        let mut sort_monitors: Vec<ScreenInfo> = monitors.to_vec();
        // Ensure order of monitors is top to bottom followed by left to right.
        sort_monitors.sort_by(|a, b| {
            let ao = a.origin;
            let bo = b.origin;
            (ao.y, ao.x).cmp(&(bo.y, bo.x))
        });
        ScreenSpace {
            range,
            monitors: sort_monitors,
        }
    }

    pub fn range(&self) -> Range {
        self.range
    }
    pub fn monitors(&self) -> &[ScreenInfo] {
        &self.monitors
    }
}
