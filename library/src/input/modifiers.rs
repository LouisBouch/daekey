//! Exposes bit flags for different modifiers.

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
