//! Holds keybind related logic and struts.

pub(crate) mod builder;
mod keybind;
mod action;

// Re-export what the user will need to make bindings.
pub use action::BuiltinAction;
pub use builder::{PendingRunnable, BindingsBuilder};




