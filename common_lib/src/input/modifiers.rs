//! Exposes bit flags for different modifiers.

use std::{fmt::Debug, ops::BitOr};

use serde::{Deserialize, Serialize};

/// What [`Modifiers`] to use to match to bindind.
#[derive(Serialize, Deserialize, Debug, Eq, Hash, PartialEq, Copy, Clone)]
pub enum ModifierMatch {
    /// At lest the defined [`Modifiers`] should match.
    AtLeast(Modifiers),
    /// Exact set of [`Modifiers`] need to match.
    Exact(Modifiers),
}

/// Represents a set of modifiers.
#[derive(Serialize, Deserialize, Eq, Hash, PartialEq, Copy, Clone)]
/// List of active modifiers stored as a bitmap internally.
pub struct Modifiers(u16);
impl Modifiers {
    /// Returns `true` if the current [`Modifiers`] are contained within the given [`Modifiers`]
    /// and `false` otherwise.
    pub const fn contains(self, modifiers: Modifiers) -> bool {
        (self.0) & (modifiers.0) == modifiers.0
    }
    /// Union two sets of [`Modifiers`].
    pub const fn union(self, modifiers: Modifiers) -> Modifiers {
        Modifiers(self.0 | modifiers.0)
    }
    /// Intersect two sets of [`Modifiers`].
    pub const fn intersect(self, modifiers: Modifiers) -> Modifiers {
        Modifiers(self.0 & modifiers.0)
    }
    /// Subtract one set of [`Modifiers`] from another.
    ///
    /// # Example
    /// ```rust
    /// use common_lib::input::Modifiers;
    ///
    /// let m = Modifiers::LEFT_SHIFT.union(Modifiers::LEFT_CTRL).subtract(Modifiers::LEFT_SHIFT);
    /// assert_eq!(m, Modifiers::LEFT_CTRL);
    /// ```
    pub const fn subtract(&self, modifiers: Modifiers) -> Modifiers {
        Modifiers(self.0 & (!modifiers.0))
    }
    /// Return `true` if it is [`Modifiers::NONE`], `false` otherwise.
    pub const fn is_empty(self) -> bool {
        self.0 == Self::NONE.0
    }
    /// Get the amount of modifiers present.
    pub fn modifier_count(&self) -> u32 {
        self.0.count_ones()
    }
    /// Get the bitmap representation of the [`Modifiers`].
    pub(crate) fn bits(&self) -> u16 {
        self.0
    }
}
/// Expose default values for the modifiers.
impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);

    pub const LEFT_SHIFT: Modifiers = Modifiers(1 << 0);
    pub const RIGHT_SHIFT: Modifiers = Modifiers(1 << 1);

    pub const LEFT_CTRL: Modifiers = Modifiers(1 << 2);
    pub const RIGHT_CTRL: Modifiers = Modifiers(1 << 3);

    pub const LEFT_ALT: Modifiers = Modifiers(1 << 4);
    pub const RIGHT_ALT: Modifiers = Modifiers(1 << 5);

    pub const LEFT_META: Modifiers = Modifiers(1 << 6);
    pub const RIGHT_META: Modifiers = Modifiers(1 << 7);
    const MODIFIERS_LIST: [(Modifiers, &str); 8] = [
        (Self::LEFT_SHIFT, "Left_Shift"),
        (Self::RIGHT_SHIFT, "Right_Shift"),
        (Self::LEFT_ALT, "Left_Alt"),
        (Self::RIGHT_ALT, "Right_Alt"),
        (Self::LEFT_META, "Left_Meta"),
        (Self::RIGHT_META, "Right_Meta"),
        (Self::LEFT_CTRL, "Left_Ctrl"),
        (Self::RIGHT_CTRL, "Right_Ctrl"),
    ];
}
impl BitOr for Modifiers {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        Modifiers(self.0 | rhs.0)
    }
}
impl Debug for Modifiers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Modifiers(")?;
        let mut first = true;
        for (modi, string_modi) in Self::MODIFIERS_LIST {
            if self.contains(modi) {
                if !first {
                    f.write_str(" | ")?;
                }
                f.write_str(string_modi)?;
                first = false;
            }
        }
        f.write_str(")")
    }
}
