use serde::{Deserialize, Serialize};

/// State of a key.

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
#[repr(i32)]
/// The state of a key.
pub enum KeyState {
    /// Key is released.
    Released = 0,
    /// Key is pressed.
    Pressed = 1,
}
