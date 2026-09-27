//! This module is deployed when processes launch the daemon with a specific flag.

/// Connects to the daemon socket and return to the calling process the newly created socket with
/// SCM_RIGHTS.
pub fn launch_connection_helper() {
}
