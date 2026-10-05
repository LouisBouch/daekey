//! This module is deployed when processes launch the daemon with a specific flag.

use std::io::IoSlice;
use std::{
    error::Error,
    fmt::Display,
    os::{fd::AsRawFd, unix::net::UnixStream},
    path::PathBuf,
};

use nix::sys::socket::{self, ControlMessage, MsgFlags};

/// Errors that can occur when connecting and passing the socket using the helper.
#[derive(Debug)]
pub enum ConnectionHelperError {
    /// Error when trying to connect to the socket.
    Connection(std::io::Error, PathBuf),
    /// Failed to send the socket using SCM_RIGHTS.
    SocketPassing(nix::errno::Errno),
}
impl Display for ConnectionHelperError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionHelperError::Connection(_, path) => {
                write!(f, "failed to connect to the daemon's socket at `{path:?}`")
            }
            ConnectionHelperError::SocketPassing(_) => {
                write!(f, "failed to send the new socket using SCM_RIGHTS")
            }
        }
    }
}
impl Error for ConnectionHelperError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConnectionHelperError::Connection(error, _) => Some(error),
            ConnectionHelperError::SocketPassing(errno) => Some(errno),
        }
    }
}

/// Connects to the daemon socket and return to the calling process the newly created socket with
/// SCM_RIGHTS.
pub fn run_connection_helper() -> Result<(), ConnectionHelperError> {
    let socket_path = PathBuf::from(libdae::constants::DAEMON_SOCKET_PATH);
    let socket = UnixStream::connect(socket_path.clone())
        .map_err(|e| ConnectionHelperError::Connection(e, socket_path))?;
    let socket_fd: std::os::fd::RawFd = socket.as_raw_fd();
    let payload = [0u8];
    let iov = [IoSlice::new(&payload)];
    let cmsg = [ControlMessage::ScmRights(&[socket_fd])];
    let _ = socket::sendmsg::<()>(
        std::io::stdout().as_raw_fd(),
        &iov,
        &cmsg,
        MsgFlags::empty(),
        None,
    )
    .map_err(ConnectionHelperError::SocketPassing)?;
    Ok(())
}
// TODO: Handle the starting of the helper from the library.
