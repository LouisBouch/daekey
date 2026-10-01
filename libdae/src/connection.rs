//! Handles the connection to the daemon.

use std::{os::{fd::{FromRawFd, IntoRawFd}, unix::net::UnixStream}, process::{Command, Stdio}};

use crate::constants;

const SOCKET_OWNER_NAME: &str = "daekey";


pub fn connect_to_daemon() {
    let (socket_core_end, socket_daemon_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let fd_socket_daemon_end=socket_daemon_end.into_raw_fd();
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
        .expect("command should not error out");
    // TODO: Block receive on the socket and return a result with a UnixStream or custom error.
    // fn get_socket() -> std::io::Result<WrappedSocketBag> {
    //     let mut payload = [0u8];
    //     let mut iov = [IoSliceMut::new(&mut payload)];
    //     let mut cmsg_buffer = nix::cmsg_space!([std::os::unix::io::RawFd; 1]);
    //     let msg = recvmsg::<()>(
    //         std::io::stdin().as_raw_fd(),
    //         &mut iov,
    //         Some(&mut cmsg_buffer),
    //         MsgFlags::empty(),
    //     )?;
    //     let mut socket = None;
    //     for cmsg in msg.cmsgs()? {
    //         if let ControlMessageOwned::ScmRights(fds) = cmsg {
    //             for fd in fds {
    //                 unsafe {
    //                     socket = Some(UnixStream::from_raw_fd(fd));
    //                 }
    //             }
    //         }
    //     }
    //     let socket = socket.expect("socket should have been obtained");
    //     let st = SocketType::try_from(payload[0] as i8)
    //         .expect("integer should convert ot valid [`SocketType`]");
    //     Ok(WrappedSocketBag::from_socket_type(socket, st, 256))
    // }
}
