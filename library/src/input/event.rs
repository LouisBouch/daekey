//! Device event that the program received.

use serde::{Deserialize, Serialize};
use evdev::KeyCode;

use crate::input::modifiers::Modifiers;

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
#[repr(i32)]
/// The state of a key.
pub enum KeyState {
    /// Key is released.
    Released = 0,
    /// Key is pressed.
    Pressed = 1,
    /// Key is being held down.
    Repeated = 2,
}

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Represents a key's activation with its context/modifiers.
pub struct KeyEvent {
    /// Scancode of the key.
    pub code: KeyCode,
    /// State of the key.
    pub state: KeyState,
    /// Modifiers present when event triggered.
    pub modifiers: Modifiers,
}
impl KeyEvent {
    /// Create a new [`KeyEvent`].
    pub fn new(code: KeyCode, state: KeyState, modifiers: Modifiers) -> Self {
        KeyEvent {
            code,
            state,
            modifiers,
        }
    }
}
