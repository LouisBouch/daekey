//! Handles the connection to the daemon.

use std::{
    error::Error,
    fmt::Display,
    io::IoSliceMut,
    os::{
        fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd},
        unix::net::UnixStream,
    },
    process::{Command, Stdio},
    time::Duration,
};

use nix::{
    errno::Errno,
    sys::socket::{ControlMessageOwned, MsgFlags, recvmsg},
};

use crate::constants;

const SOCKET_OWNER_NAME: &str = "daekey";
const CONNECTION_TIMEOUT_MILLIS: u64 = 1000;
/// Error when connecting to the daemon and obtaining a connection socket.
#[derive(Debug)]
pub enum ConnectionError {
    /// Failed to create initial socket pair.
    SocketPairCreation(std::io::Error),
    /// Failed to launch the daemon subcommand helper.
    SubcommandLaunch(std::io::Error),
    /// Failed to wait for the subcommand to finish.
    SubcommandWait(std::io::Error),
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
            ConnectionError::SubcommandWait(_) => {
                write!(f, "failed to wait for the daemon's subcommand")
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
            ConnectionError::SubcommandWait(error) => Some(error),
            ConnectionError::SocketAcquisition(socket_acquisition_error) => {
                Some(socket_acquisition_error)
            }
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
impl Display for SocketAcquisitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAcquisitionError::ReceiveMessage(_) => {
                write!(
                    f,
                    "failed to receive a message from the daemon's subcommand"
                )
            }
            SocketAcquisitionError::IterateMessages(_) => {
                write!(
                    f,
                    "failed to iterate over the messages received from the daemon's subcommand"
                )
            }
            SocketAcquisitionError::Timedout => {
                write!(
                    f,
                    "message reception from the daemon's subcommand timed out"
                )
            }
            SocketAcquisitionError::TooManyFds => {
                write!(f, "daemon's subcommand returned to many file descriptors")
            }
            SocketAcquisitionError::MissingSocket => {
                write!(
                    f,
                    "daemon's subcommand failed to return a file descriptor, it might have crashed"
                )
            }
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
fn wait_for_socket(socket_core_end: UnixStream) -> Result<UnixStream, SocketAcquisitionError> {
    // Receive socket from spawned process.
    let mut payload = [0u8];
    let mut iov = [IoSliceMut::new(&mut payload)];
    let mut cmsg_buffer = nix::cmsg_space!([std::os::unix::io::RawFd; 1]);
    socket_core_end
        .set_read_timeout(Some(Duration::from_millis(CONNECTION_TIMEOUT_MILLIS)))
        .expect("should be able to set timeout");
    let msg = recvmsg::<()>(
        socket_core_end.as_raw_fd(),
        &mut iov,
        Some(&mut cmsg_buffer),
        MsgFlags::empty(),
    )
    .map_err(|e| match e {
        Errno::EAGAIN => SocketAcquisitionError::Timedout,
        _ => SocketAcquisitionError::ReceiveMessage(e),
    })?;
    let mut socket = None;
    let cmsgs = msg
        .cmsgs()
        .map_err(SocketAcquisitionError::IterateMessages)?;
    let mut too_many_fds = false;
    for cmsg in cmsgs {
        if let ControlMessageOwned::ScmRights(fds) = cmsg {
            for fd in fds {
                if socket.is_some() {
                    // Don't exit now to prevent remaining fds from leaking.
                    // Wrap and drop all of them and exit with error later.
                    too_many_fds = true;
                }
                unsafe {
                    socket = Some(UnixStream::from_raw_fd(fd));
                }
            }
        }
    }
    if too_many_fds {
        return Err(SocketAcquisitionError::TooManyFds);
    }
    socket.ok_or(SocketAcquisitionError::MissingSocket)
}

/// Connect to the daemon through its subcommand helper and obtain a connection socket.
pub fn socket_from_daemon() -> Result<UnixStream, ConnectionError> {
    let (socket_core_end, socket_daemon_end) =
        std::os::unix::net::UnixStream::pair().map_err(ConnectionError::SocketPairCreation)?;
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
            "helper"
        ])
        .stdout(Stdio::from(OwnedFd::from(socket_daemon_end)))
        .spawn()
        .map_err(ConnectionError::SubcommandLaunch)?
        .wait()
        .map_err(ConnectionError::SubcommandWait)?;
    wait_for_socket(socket_core_end).map_err(ConnectionError::SocketAcquisition)
}
