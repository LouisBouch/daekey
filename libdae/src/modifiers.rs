//! Exposes bit flags for different modifiers.
use evdev::{AttributeSet, KeyCode};

use crate::modifiers;

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

const MOD_LIST: [(Modifiers, KeyCode); 8] = [
    (LEFT_SHIFT, KeyCode::KEY_LEFTSHIFT),
    (RIGHT_SHIFT, KeyCode::KEY_RIGHTSHIFT),
    (LEFT_CTRL, KeyCode::KEY_LEFTCTRL),
    (RIGHT_CTRL, KeyCode::KEY_RIGHTCTRL),
    (LEFT_ALT, KeyCode::KEY_LEFTALT),
    (RIGHT_ALT, KeyCode::KEY_RIGHTALT),
    (LEFT_META, KeyCode::KEY_LEFTMETA),
    (RIGHT_META, KeyCode::KEY_RIGHTMETA),
];


/// Obtain a list of modifiers' keycode from a modifiers bit flag.
pub fn keycodes_from_modifiers(modifiers: Modifiers) -> Vec<KeyCode> {
    let mut codes = Vec::new();
    for (modifier, code) in MOD_LIST {
        if modifier & modifiers != 0 {
            codes.push(code);
        }
    }
    codes
}
/// Obtain the modifiers from a list of key codes.
pub fn modifiers_from_key_codes(set: &AttributeSet<KeyCode>) -> Modifiers {
    let mut modifiers: modifiers::Modifiers = modifiers::NONE;
    for (modi, code) in MOD_LIST {
        if set.contains(code) {
            modifiers |= modi;
        }
    }
    modifiers
}
/// Get modifier from a keycode, returns 0 if not a modifier.
pub fn modifier_from_keycode(key: KeyCode) -> Modifiers {
    for (modi, code) in MOD_LIST {
        if key == code {
            return modi;
        }
    }
    modifiers::NONE
}
