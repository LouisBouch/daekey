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

// TODO: Make privileged process a daemon. Have user launch individual processes that request
// connection through a binary that requries sudo. Once connected, the new user launched app can
// communicate with the daemon.
// Each connection has a single connection to the daemon, and all requests go through it.
// Make the daemon listen to compositor change and have it send the updated values to the list of connections.
// Upon connecting, have the daemon send compositor values (like the screen layout an stuff) to the
// processes connecting to it.

// TODO: Create macro maker. Fetches initiale absolute cursor position on a key press and then
// record everything that is done. But before doing it, clean up the code a bit.


// TODO: instead of just hoding the binding in a hashmap, hold them in a trie?
