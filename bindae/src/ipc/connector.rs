//! Defines how the daemon accepts connections from other processes.

use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::Sender;

/// Create the socket server other processes will use to connect to this daemon.
pub fn create_socket_server() -> std::io::Result<UnixListener> {
    let path_to_socket = PathBuf::from(libdae::constants::DAEMON_SOCKET_PATH);
    // Remove old socket if it wasn't properly removed last time.
    let _ = std::fs::remove_file(&path_to_socket);
    UnixListener::bind(&path_to_socket)
}

/// Listens for new connections on the socket server and send them over the given channel.
///
/// # Parameters
///
/// * `socket_server` - The server listening for new connections to the daemon.
/// * `connection_tx` - The channel to send new connection sockets over.
///
/// # Return
///
/// The handle for the thread listening for new connections.
pub fn listen_socket_server(
    socket_server: UnixListener,
    connection_tx: Sender<UnixStream>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut subsequent_stream_err = 0;
        for stream in socket_server.incoming() {
            match stream {
                Ok(stream) => {
                    subsequent_stream_err = 0;
                    if let Err(e) = connection_tx.send(stream) {
                        eprintln!(
                            "Failed to send new connection, stopping connection listener: {e}"
                        );
                        break;
                    };
                }
                Err(e) => {
                    subsequent_stream_err += 1;
                    eprintln!("Failed to get new connection: {e}");
                    if subsequent_stream_err == 100 {
                        eprintln!("Too many subsequent stream errors, stopping connection listener");
                        break;
                    }
                    // Sleep to prevent fast error loop.
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    })
}
