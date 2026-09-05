pub mod configs;
#[doc(hidden)]
pub mod app;
pub mod input;
pub mod api;
#[doc(hidden)]
pub mod message;
pub mod modifiers;
#[doc(hidden)]
pub mod compositor_interface;

pub use evdev::KeyCode;
pub use evdev::{RelativeAxisCode, AbsoluteAxisCode};
pub use message::AppliedModifiers;
pub type Pixel = i32;

// TODO: Create macro maker. Fetches initiale absolute cursor position on a key press and then
// record everything that is done. But before doing it, clean up the code a bit.


// TODO: instead of just hoding the binding in a hashmap, hold them in a trie?
