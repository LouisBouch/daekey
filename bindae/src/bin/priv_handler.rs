mod manager;
use crossbeam_channel::Sender;
use libdae::{
    app::{SocketType, WorkerPrivSocket, WrappedSocketBag},
    message,
};
use manager::{input_manager, uinput_manager};
use std::{
    io::IoSliceMut,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::net::UnixStream,
    },
    thread::{self, JoinHandle},
};

use nix::sys::socket::{ControlMessageOwned, MsgFlags, recvmsg};

struct PrivHandler {}
impl PrivHandler {
    pub fn launch_handler() {
        // Read necessary data first.

        let ctx: libdae::app::SetupContext = postcard::from_io((std::io::stdin(), &mut [0; 256]))
            .unwrap()
            .0;
        // Use fd 0 as a write-back stream (works because its a socket) instead of stdin.
        let socket_stream = unsafe { UnixStream::from_raw_fd(0) };
        postcard::to_io(&true, &socket_stream).expect("postcard should be able to serialize");
        // Ignore the socket and keep using stdin.
        std::mem::forget(socket_stream);

        let mut input_socket = None;
        let mut uinput_socket = None;
        let mut worker_sockets = Vec::new();
        let sockets =
            Self::get_sockets(ctx.nb_threads()).expect("sockets should be read successfully");
        for socket in sockets {
            match socket {
                WrappedSocketBag::WorkerPriv(wrapped_socket) => worker_sockets.push(wrapped_socket),
                WrappedSocketBag::InputPriv(wrapped_socket) => input_socket = Some(wrapped_socket),
                WrappedSocketBag::UInputPriv(wrapped_socket) => {
                    uinput_socket = Some(wrapped_socket)
                }
                _ => eprintln!(
                    "There should not be any core socket in the privileged process: {socket:?}"
                ),
            }
        }
        let (input_socket, uinput_socket) = (
            input_socket.expect("input_socket should be initialized"),
            uinput_socket.expect("uinput_socket should be initialized"),
        );

        let uinput_share =
            uinput_manager::launch_uinput_listener(uinput_socket, ctx.screen_space())
                .expect("uinput manager should launch successfully");
        let input_share = input_manager::launch_input_listener(
            input_socket,
            uinput_share.uinput_sender().clone(),
            ctx.min_mouse_poll_interval(),
        )
        .expect("input manager should launch successfully");

        // Spin up workers.
        let handles = Self::launch_workers(worker_sockets, uinput_share.uinput_sender());

        // Listen for parent death.
        thread::spawn(|| {
            let stdin = &std::io::stdin();
            loop {
                let mut buf = [0; 256];
                let mes_res: postcard::Result<([u8; 1], _)> = postcard::from_io((stdin, &mut buf));
                match &mes_res {
                    Ok(_) => eprintln!("Received unexpected byte from stdin"),
                    Err(e) => match e {
                        postcard::Error::DeserializeUnexpectedEnd => {
                            eprintln!("parent process died, killing current process: '{e}'");
                            std::process::exit(1);
                        }
                        _ => {
                            eprintln!(
                                "unexpected error, could not read from socket, killing current process: '{e}'"
                            );
                            std::process::exit(1);
                        }
                    },
                };
            }
        });

        input_share.join();
        uinput_share.join();
        for (i, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(_) => (),
                Err(e) => eprintln!("Worker thread {i} panicked: {e:?}"),
            }
        }
    }
    fn launch_workers(
        worker_sockets: Vec<WorkerPrivSocket>,
        uinput_sender: &Sender<message::MsgToUInput>,
    ) -> Vec<JoinHandle<()>> {
        let mut handlers = Vec::new();
        for (i, mut socket) in worker_sockets.into_iter().enumerate() {
            let uinput_sender = uinput_sender.clone();
            let handle = thread::spawn(move || {
                loop {
                    let mes_res = socket.receive();
                    let mes: message::MsgToWorker = match mes_res {
                        Ok(v) => v,
                        Err(e) => match e {
                            postcard::Error::DeserializeUnexpectedEnd => {
                                eprintln!("parent process died, terminating worker {i} : '{e}'");
                                break;
                            }
                            _ => {
                                eprintln!(
                                    "unexpected error, could not read from socket, terminating worker {i}: '{e}'"
                                );
                                break;
                            }
                        },
                    };
                    match mes {
                        message::MsgToWorker::UInputRequest(uinput_msg) => {
                            message::send_msg(&uinput_sender, uinput_msg);
                        }
                    }
                }
            });
            handlers.push(handle);
        }
        handlers
    }

    fn get_sockets(nb_workers: u16) -> std::io::Result<Vec<WrappedSocketBag>> {
        let mut sockets = Vec::new();
        // Get worker sockets
        for _ in 0..(nb_workers) {
            sockets.push(Self::get_socket()?);
        }
        // Get input socket.
        sockets.push(Self::get_socket()?);
        // Get uinput socket.
        sockets.push(Self::get_socket()?);
        Ok(sockets)
    }
    fn get_socket() -> std::io::Result<WrappedSocketBag> {
        let mut payload = [0u8];
        let mut iov = [IoSliceMut::new(&mut payload)];
        let mut cmsg_buffer = nix::cmsg_space!([std::os::unix::io::RawFd; 1]);
        let msg = recvmsg::<()>(
            std::io::stdin().as_raw_fd(),
            &mut iov,
            Some(&mut cmsg_buffer),
            MsgFlags::empty(),
        )?;
        let mut socket = None;
        for cmsg in msg.cmsgs()? {
            if let ControlMessageOwned::ScmRights(fds) = cmsg {
                for fd in fds {
                    unsafe {
                        socket = Some(UnixStream::from_raw_fd(fd));
                    }
                }
            }
        }
        let socket = socket.expect("socket should have been obtained");
        let st = SocketType::try_from(payload[0] as i8)
            .expect("integer should convert ot valid [`SocketType`]");
        Ok(WrappedSocketBag::from_socket_type(socket, st, 256))
    }
}
fn main() {
    PrivHandler::launch_handler();
}
