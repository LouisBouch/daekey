//! Defines keybindings.

use serde::{Deserialize, Serialize};

use crate::keys::{self, event::{self, KeyEvent}, modifiers::Modifiers};

/// What [`Modifiers`] to use to match to bindind.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum ModifiersMatch {
    /// Any [`Modifiers`] will match to the binding.
    Any,
    /// Exact set of [`Modifiers`] will match to the binding.
    Exact(Modifiers),
}


#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Represents a key with its context/modifiers.
pub struct ModdedKey {
    /// A [`event::KeyCode`] from a physical device.
    key_code: keys::KeyCode,
    /// [`Modifiers`] requried to match with the key.
    modifiers: ModifiersMatch,
}
impl ModdedKey {
    /// Create a new [`ModdedKey`].
    pub fn new(key_code: keys::KeyCode, modifiers: ModifiersMatch) -> Self {
        ModdedKey {
            key_code,
            modifiers,
        }
    }
}
