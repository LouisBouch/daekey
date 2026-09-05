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

// TODO: Order of things:
//
// 1. Remove creation of multiple sockets and keep only one socket to talk between the processes.
// 2. Make the privileged process listen for compositor changes and push them to the
//    core/unprivileged process. Also, have the daemon send the initial compositor values.
// 3. Make the daemon create a single connection socket in the /run folder and only allow rooted
//    processes to access it.
// 4. Make the daemon independent from the core, that way they can be launched independently, and
//    ensure the core, which is now just a normal connection to the daemon, can find the daemon's socket.
// 5. Instead of shelling out to sudo for anything, always start with sudo privileges, and drop to
//    minimum privileges as soon as possible (after setpriv for daemon and after connection to the
//    socket for the "core" connections).

// TODO: Create macro maker. Fetches initiale absolute cursor position on a key press and then
// record everything that is done. But before doing it, clean up the code a bit.


// TODO: instead of just hoding the binding in a hashmap, hold them in a trie?
