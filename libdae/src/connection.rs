//! Handles the connection to the daemon.

use std::{
    error::Error, fmt::Display, io::IoSliceMut, os::{
        fd::{FromRawFd, IntoRawFd},
        unix::net::UnixStream,
    }, process::{Command, Stdio}
};

use nix::{
    errno::Errno,
    sys::socket::{ControlMessageOwned, MsgFlags, recvmsg},
};

use crate::constants;

const SOCKET_OWNER_NAME: &str = "daekey";
/// Error when connecting to the daemon and obtaining a connection socket.
#[derive(Debug)]
pub enum ConnectionError {
    /// Failed to create initial socket pair.
    SocketPairCreation(std::io::Error),
    ///Failed to launch the daemon subcommand helper.
    SubcommandLaunch(std::io::Error),
    /// Failed to obtain a socket from the daemon.
    SocketAcquisition(SocketAcquisitionError),
}
impl Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionError::SocketPairCreation(_) => {
                write!(
                    f,
                    "failed to create a socket pair to allow commmunication with the daemon's subcommand"
                )
            }
            ConnectionError::SubcommandLaunch(_) => {
                write!(f, "failed to launch the daemon's subcommand")
            }
            ConnectionError::SocketAcquisition(_) => {
                write!(f, "failed to acquire a socket for the daemon")
            }
        }
    }
}
impl Error for ConnectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConnectionError::SocketPairCreation(error) => Some(error),
            ConnectionError::SubcommandLaunch(error) => Some(error),
            ConnectionError::SocketAcquisition(socket_acquisition_error) => Some(socket_acquisition_error),
        }
    }
}
/// An error when trying to fetch the socket from the daemon.
#[derive(Debug)]
pub enum SocketAcquisitionError {
    /// Cannot receive a message from the daemon.
    ReceiveMessage(Errno),
    /// Iterating over the messages received from the daemon failed.
    IterateMessages(Errno),
    /// Daemon did not send an answer back in time.
    Timedout,
    /// Daemon sent too many file descriptors, only 1 is expected.
    TooManyFds,
    /// Message received from daemon contained no socket.
    MissingSocket,
}
impl Display for SocketAcquisitionError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAcquisitionError::ReceiveMessage(_) => {
                write!(f, "failed to receive a message from the daemon's subcommand")
            },
            SocketAcquisitionError::IterateMessages(_) => {
                write!(f, "failed to iterate over the messages received from the daemon's subcommand")
            },
            SocketAcquisitionError::Timedout => {
                write!(f, "message reception from the daemon's subcommand timed out")
            },
            SocketAcquisitionError::TooManyFds => {
                write!(f, "daemon's subcommand returned to many file descriptors")
            },
            SocketAcquisitionError::MissingSocket => {
                write!(f, "daemon's subcommand failed to return a file descriptor")
            },
        }
    }
}
impl Error for SocketAcquisitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SocketAcquisitionError::ReceiveMessage(errno) => Some(errno),
            SocketAcquisitionError::IterateMessages(errno) => Some(errno),
            SocketAcquisitionError::Timedout => None,
            SocketAcquisitionError::TooManyFds => None,
            SocketAcquisitionError::MissingSocket => None,
        }
    }
}
/// Obtain a connection socket from the daemon subcommand helper.
fn get_socket(fd_socket_core_end: i32) -> Result<UnixStream, SocketAcquisitionError> {
    // Receive socket from spawned process.
    let mut payload = [0u8];
    let mut iov = [IoSliceMut::new(&mut payload)];
    let mut cmsg_buffer = nix::cmsg_space!([std::os::unix::io::RawFd; 1]);
    // TODO: Throw error if waiting too long and use `SocketAcquisitionError::Timedout` with it.
    let msg = recvmsg::<()>(
        fd_socket_core_end,
        &mut iov,
        Some(&mut cmsg_buffer),
        MsgFlags::empty(),
    )
    .map_err(SocketAcquisitionError::ReceiveMessage)?;

    let mut socket = None;
    let cmsgs = msg
        .cmsgs()
        .map_err(SocketAcquisitionError::IterateMessages)?;
    for cmsg in cmsgs {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            for fd in fds {
                if socket.is_some() {
                    return Err(SocketAcquisitionError::TooManyFds);
                }
                unsafe {
                    socket = Some(UnixStream::from_raw_fd(fd));
                }
            }
        }
    }
    socket.ok_or(SocketAcquisitionError::MissingSocket)
}

/// Connect to the daemon through its subcommand helper and obtain a connection socket.
pub fn socket_from_daemon() -> Result<UnixStream, ConnectionError> {
    let (socket_core_end, socket_daemon_end) =
        std::os::unix::net::UnixStream::pair().map_err(ConnectionError::SocketPairCreation)?;
    let fd_socket_daemon_end = socket_daemon_end.into_raw_fd();
    Command::new("sudo")
        .args([
            "setpriv",
            "--groups",
            "input,uinput",
            "--ruid",
            SOCKET_OWNER_NAME,
            "--rgid",
            SOCKET_OWNER_NAME,
            constants::DAEMON_BIN_PATH,
        ])
        .stdout(unsafe { Stdio::from_raw_fd(fd_socket_daemon_end) })
        .spawn()
        .map_err(ConnectionError::SubcommandLaunch)?;
    let fd_socket_core_end = socket_core_end.into_raw_fd();
    get_socket(fd_socket_core_end).map_err(ConnectionError::SocketAcquisition)
}
