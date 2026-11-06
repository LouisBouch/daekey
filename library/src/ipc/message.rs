//! Holds differents types of messages that can be sent across processes.

use crate::keys::KeyCode;
use evdev::{AbsoluteAxisCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};

use crate::keys::event::KeyState;
use crate::keys::modifiers::ModifiersMatch;

#[derive(Serialize, Deserialize, Clone, Debug)]
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
}
/// A message to the logical device from a simulated one.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FromProcess {
    /// Request actions from the logical device.
    Request(Vec<Request>),
    /// While true, disconnects selected bindings from reachingthe device.
    SetPause(bool),
    /// Process wants to subscribe to a key.
    Subscribe,
    /// Close the connection.
    Close,
}


/// A message from the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromDaemon {
}
