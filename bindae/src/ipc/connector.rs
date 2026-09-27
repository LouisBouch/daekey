//! Defines how the daemon accepts connections from other processes.

use std::error::Error;
use std::fmt::Display;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::Sender;

/// Different types of error that can occur when manipulating the socket server.
#[derive(Debug)]
pub enum SocketServerError {
    /// Error while deleting the socket.
    Delete(std::io::Error),
    /// Error while creating the socket.
    Create(std::io::Error),
    /// The directory that is supposed to host the socket has invalid permissions.
    ParentPermission,
    /// The directory that is supposed to host the socket does not exist.
    NoParent,
}
impl Display for SocketServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketServerError::Delete(error) => {
                write!(f, "Error while deleting an old connection socket: {error}")
            }
            SocketServerError::Create(error) => {
                write!(f, "Error while creating a connection socket: {error}")
            }
            SocketServerError::ParentPermission => {
                write!(f, "Socket's parent directory has invalid permissions or groups. Should be daekey:daekey, 750.")
            },
            SocketServerError::NoParent => {
                write!(f, "Socket's parent directory does not exist. Try restarting the service?")
            },
        }
    }
}
impl Error for SocketServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SocketServerError::Delete(error) => Some(error),
            SocketServerError::Create(error) => Some(error),
            SocketServerError::ParentPermission => None,
            SocketServerError::NoParent => None,
        }
    }
}

/// Create the socket server other processes will use to connect to this daemon.
pub fn create_socket_server() -> Result<UnixListener, SocketServerError> {
    let path_to_socket = PathBuf::from(libdae::constants::DAEMON_SOCKET_PATH);
    // Remove old socket if it wasn't properly removed last time.
    if let Err(e) = std::fs::remove_file(&path_to_socket) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(SocketServerError::Delete(e));
        }
    }
    // TODO: Ensure the diretory that the socket will be created in has correct permissions.
    // daekey : daekey, 750
    UnixListener::bind(&path_to_socket).map_err(SocketServerError::Create)
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
                        eprintln!(
                            "Too many subsequent stream errors, stopping connection listener"
                        );
                        break;
                    }
                    // Sleep to prevent fast error loop.
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
    })
}
