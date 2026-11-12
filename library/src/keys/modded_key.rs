//! Defines keybindings.

use serde::{Deserialize, Serialize};

use crate::keys::{
    self,
    modifiers::ModifierMatch,
};

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Represents a key with its context/modifiers.
pub struct ModdedKey {
    /// A [`event::KeyCode`] from a physical device.
    key_code: keys::KeyCode,
    /// [`Modifiers`] requried to match with the key.
    modifiers: ModifierMatch,
}
impl ModdedKey {
    /// Create a new [`ModdedKey`].
    pub fn new(key_code: keys::KeyCode, modifiers: ModifierMatch) -> Self {
        ModdedKey {
            key_code,
            modifiers,
        }
    }
}
