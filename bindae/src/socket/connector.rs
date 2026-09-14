//! Defines how the daemon accepts connections from other processes.

use std::os::unix::net::UnixListener;
use std::path::PathBuf;

/// Create the socket server other processes will use to connect to this daemon.
pub fn create_connection_socket() -> std::io::Result<UnixListener> {
    let path_to_socket = PathBuf::new().join(libdae::constants::DAEMON_SOCKET_PATH);
    // Remove old socket if it wasn't properly removed last time.
    let _ = std::fs::remove_file(path_to_socket.clone());
    UnixListener::bind(path_to_socket)
}
