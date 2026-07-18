//! Holds different variations of key representation depending on the context.
use evdev::KeyCode;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::modifiers::Modifiers;
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
#[repr(i32)]
pub enum KeyState {
    Released = 0,
    Pressed = 1,
    Repeated = 2,
}

#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Represents the key being activated alongside its modifiers (like Shift, Ctrl, etc...).
pub struct Keybind {
    pub code: KeyCode,
    pub state: KeyState,
    pub modifiers: Modifiers,
}
impl Keybind {
    pub fn new(code: KeyCode, state: KeyState, modifiers: Modifiers) -> Self {
        Keybind {
            code,
            state,
            modifiers,
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
/// Defines a simple key action that can be sent to the compositor.
pub struct KeyAction {
    pub key: KeyCode,
    pub state: KeyState,
}

impl KeyAction {
    pub fn new(key: KeyCode, state: KeyState) -> Self {
        KeyAction { key, state }
    }
}

