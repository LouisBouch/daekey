//! Handles the link to the app's API.

use std::os::unix::net::UnixStream;

/// Used as an API for the app's functionalities.
pub struct Handle {
    /// Socket connected to the daemon.
    daemon_socket: UnixStream,
}
impl Handle {
    pub fn new(daemon_socket: UnixStream) -> Self {
        Handle { daemon_socket }
    }
}
//TODO: Implement
