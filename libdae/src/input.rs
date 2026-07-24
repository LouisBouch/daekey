//! Holds different variations of key representation depending on the context.
use evdev::{AbsoluteAxisCode, KeyCode, RelativeAxisCode};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use crate::{Pixel, modifiers::Modifiers};
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

/// Defines a simple relative mouse action.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub struct MouseRelAction {
    /// Along which axis to move.
    pub axis: RelativeAxisCode,
    /// How many pixels to shift.
    pub value: Pixel,
}

impl MouseRelAction {
    pub fn new(axis: RelativeAxisCode, value: i32) -> Self {
        MouseRelAction { axis, value }
    }
}
/// Defines a simple relative mouse action.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub struct MouseAbsAction {
    /// Along which axis to move.
    pub axis: AbsoluteAxisCode,
    /// Where to move along the axis.
    pub pos: i32,
}

impl MouseAbsAction {
    pub fn new(axis: AbsoluteAxisCode, pos: i32) -> Self {
        MouseAbsAction { axis, pos }
    }
}
/// A mouse action. Either a key action or a relative motion.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum MouseAction {
    /// Absolute motion of the mouse.
    Abs(MouseAbsAction),
    /// Relative motion of the mouse.
    Rel(MouseRelAction),
    /// Button action of the mouse.
    Key(KeyAction),
}
