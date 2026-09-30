//! Defines how the daemon accepts connections from other processes.

use std::error::Error;
use std::fmt::Display;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::Sender;
use nix::unistd;

/// Required permissions for the socket's parent.
const SOCKET_PARENT_PERMISSIONS: u32 = 0o750;
/// Required group id and user id for the socket's parent.
const SOCKET_OWNER_NAME: &str = "daekey";

/// Different types of error that can occur when manipulating the socket server.
#[derive(Debug)]
pub enum SocketServerError {
    /// Error while deleting the socket.
    Delete(std::io::Error),
    /// Error while creating the socket.
    Create(std::io::Error),
    /// Failed to fetch the group or user id from the system.
    IdLookup(nix::errno::Errno),
    /// Failed to fetch the parent's directory metadata.
    MetadataLookup(PathBuf, std::io::Error),
    /// The socket's parent directory has invalid permissions.
    ParentPermission(PathBuf),
    /// The socket's parent does not exist.
    NoParent(PathBuf),
    /// The path suposed to represent the exact location of the socket is instead a directory.
    SocketPathIsDir(PathBuf),
    /// The required group or user does not exist. It should have been created by the service.
    RequiredUserOrGroup,
}
impl Display for SocketServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketServerError::Delete(_) => {
                write!(f, "failed to delete an old connection socket")
            }
            SocketServerError::Create(_) => {
                write!(f, "failed to create a connection socket")
            }
            SocketServerError::IdLookup(_) => {
                write!(
                    f,
                    "failed to fetch the user or group id `{SOCKET_OWNER_NAME}` from the system"
                )
            }
            SocketServerError::MetadataLookup(path, _) => {
                write!(
                    f,
                    "failed to fetch the metadata from the parent directory `{path:?}`"
                )
            }
            SocketServerError::ParentPermission(path) => {
                write!(
                    f,
                    "the socket's parent directory `{path:?}` has invalid permissions or groups, should be daekey:daekey, 750"
                )
            }
            SocketServerError::NoParent(path) => {
                write!(
                    f,
                    "cannot find the socket's parent directory `{path:?}`, try restarting the service"
                )
            }
            SocketServerError::SocketPathIsDir(path) => {
                write!(
                    f,
                    "socket's path `{path:?}` is a directory, it should be a path to a file"
                )
            }
            SocketServerError::RequiredUserOrGroup => {
                write!(
                    f,
                    "the required user or group `{SOCKET_OWNER_NAME}` does not exist, it should have been created by the service"
                )
            },
        }
    }
}
impl Error for SocketServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SocketServerError::Delete(error) => Some(error),
            SocketServerError::Create(error) => Some(error),
            SocketServerError::IdLookup(error) => Some(error),
            SocketServerError::MetadataLookup(_, error) => Some(error),
            SocketServerError::ParentPermission(_) => None,
            SocketServerError::NoParent(_) => None,
            SocketServerError::SocketPathIsDir(_) => None,
            SocketServerError::RequiredUserOrGroup => None,
        }
    }
}

/// Create the socket server other processes will use to connect to this daemon.
pub fn create_socket_server() -> Result<UnixListener, SocketServerError> {
    let socket_path = PathBuf::from(libdae::constants::DAEMON_SOCKET_PATH);
    // Check if path is directory.
    if socket_path.is_dir() {
        return Err(SocketServerError::SocketPathIsDir(socket_path));
    }

    // Ensure parent exists.
    let parent_path = socket_path
        .parent()
        .ok_or(SocketServerError::NoParent(socket_path.clone()))?;

    // Check the permissions, uid and gid of the parent dir.
    let exp_uid = unistd::User::from_name(SOCKET_OWNER_NAME)
        .map_err(SocketServerError::IdLookup)?
        .ok_or(SocketServerError::RequiredUserOrGroup)?.uid.as_raw();
    let exp_gid = unistd::Group::from_name(SOCKET_OWNER_NAME)
        .map_err(SocketServerError::IdLookup)?
        .ok_or(SocketServerError::RequiredUserOrGroup)?.gid.as_raw();
    let md = fs::metadata(parent_path)
        .map_err(|e| SocketServerError::MetadataLookup(parent_path.to_path_buf(), e))?;
    let uid = md.uid();
    let gid = md.gid();
    let permissions = md.permissions().mode();
    if uid != exp_uid || gid != exp_gid || (permissions & 0o7777) != SOCKET_PARENT_PERMISSIONS{
        return Err(SocketServerError::ParentPermission(parent_path.to_path_buf()));
    }

    // Remove old socket if it wasn't properly removed last time.
    if let Err(e) = std::fs::remove_file(&socket_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(SocketServerError::Delete(e));
        }
    }

    // Create the socket.
    UnixListener::bind(&socket_path).map_err(SocketServerError::Create)
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
