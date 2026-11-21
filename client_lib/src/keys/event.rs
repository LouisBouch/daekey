//! Device event that the program received.

use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
#[repr(i32)]
/// The state of a key.
pub enum KeyState {
    /// Key is released.
    Released = 0,
    /// Key is pressed.
    Pressed = 1,
}

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Represents a key's activation with its context/modifiers.
pub struct KeyEvent {
    /// Scancode of the key.
    pub code: super::KeyCode,
    /// State of the key.
    pub state: KeyState,
}
impl KeyEvent {
    /// Create a new [`KeyEvent`].
    pub fn new(code: super::KeyCode, state: KeyState) -> Self {
        Self { code, state }
    }
}
