//! Holds differents types of messages that can be sent across processes.

use crate::input::{KeyCode, RelativeAxisCode, AbsoluteAxisCode, KeyState, Modifiers};
use crate::bindings::Bindings;
use serde::{Deserialize, Serialize};


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
        modifiers: Modifiers,
    },
    /// Send a relative value motion. (Ex. Mouse movement/scroll)
    SendRelativeMotion {
        relative_axis: RelativeAxisCode,
        value: i32,
    },
    /// Send an absolute value motion. (Ex. Mouse movement to specific point on screen)
    SendAbsoluteMotion {
        absolute_axis: AbsoluteAxisCode,
        value: i32,
    },
}
/// A message to the daemon from a connected client.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum FromClient {
    /// Send requests to daemon.
    Requests(Vec<Request>),
    /// If `true`, it essentially unbinds every key.
    SetPause(bool),
    /// Set bindings for the client.
    SetBindings(Bindings),
    /// Close the connection.
    Close,
}


/// A message from the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromDaemon {
}
