//! Exposes bit flags for different modifiers.

use serde::{Deserialize, Serialize};


/// What [`Modifiers`] to use to match to bindind.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum ModifierMatch {
    /// Any [`Modifiers`] will match to the binding.
    Any,
    /// Exact set of [`Modifiers`] will match to the binding.
    Exact(Modifiers),
}

pub type Modifiers = u16;
pub const NONE: Modifiers = 0b0;

pub const LEFT_SHIFT: Modifiers = 0b0000_0000_0000_0001;
pub const RIGHT_SHIFT: Modifiers = 0b0000_0000_0000_0010;

pub const LEFT_CTRL: Modifiers = 0b0000_0000_0000_0100;
pub const RIGHT_CTRL: Modifiers = 0b0000_0000_0000_1000;

pub const LEFT_ALT: Modifiers = 0b0000_0000_0001_0000;
pub const RIGHT_ALT: Modifiers = 0b0000_0000_0010_0000;

pub const LEFT_META: Modifiers = 0b0000_0000_0100_0000;
pub const RIGHT_META: Modifiers = 0b0000_0000_1000_0000;
