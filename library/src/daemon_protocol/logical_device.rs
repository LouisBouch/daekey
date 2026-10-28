//! Communication rules with the daemon's logical device.

use crate::keys::KeyCode;
use evdev::{AbsoluteAxisCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};

use crate::keys::event::KeyState;
use crate::keys::modifiers::Modifiers;

/// What [`Modifiers`] to use to match to bindind.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum ModifiersMatch {
    /// Any [`Modifiers`] will match to the binding.
    Any,
    /// Exact set of [`Modifiers`] will match to the binding.
    Exact(Modifiers),
}

#[derive(Clone, Debug)]
/// A request to the logical device.
pub enum Request {
    /// Send a key with a specific [`KeyState`].
    SendKey {
        key_code: KeyCode,
        key_state: KeyState,
    },
    /// Send a quick key press and then release.
    SendKeyTap {
        key_code: KeyCode,
        modifiers: ModifiersMatch,
    },
    /// Send a relative value motion. (Ex. Mouse movement/scroll)
    SendRelativeMotion {
        relative_axis: RelativeAxisCode,
        value: i32,
    },
    /// Send an value value motion. (Ex. Mouse movement to specific point on screen)
    SendAbsoluteMotion {
        absolute_axis: AbsoluteAxisCode,
        value: i32,
    },
    /// Append a SYN_REPORT to every virtual device with pending requests.
    Sync,
}
/// A message to the logical device from a simulated one.
#[derive(Clone, Debug)]
pub enum SimulatedMessage {
    /// Request actions from the logical device.
    Request(Vec<Request>),
    /// While true, disconnects selected bindings from reachingthe device.
    SetPause(bool),
    /// Close the connection.
    Close,
}

/// A message to the logical device from a physical one.
#[derive(Clone, Debug)]
pub enum PhysicalMessage {
    /// Request actions from the logical device.
    Request(Vec<Request>),
    /// Close the connection.
    Close,
}
