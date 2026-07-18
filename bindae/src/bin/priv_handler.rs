mod manager;
use crossbeam_channel::Sender;
use libdae::{keys::Keybind, message};
use manager::{input_manager, uinput_manager};
use std::{
    collections::HashSet,
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

        let ctx: libdae::binder::SetupContext =
            postcard::from_io((std::io::stdin(), &mut [0; 256]))
                .unwrap()
                .0;
        // Use fd 0 as a write-back stream (works because its a socket) instead of stdin.
        let socket_stream = unsafe { UnixStream::from_raw_fd(0) };
        postcard::to_io(&true, &socket_stream).expect("postcard should be able to serialize");
        // Ignore the socket and keep using stdin.
        std::mem::forget(socket_stream);

        let (input_socket, worker_sockets) = Self::get_sockets(ctx.nb_threads()).unwrap();

        // Use read data.
        let uinput_share = uinput_manager::launch_virtual_device()
            .expect("uinput manager shoudl launch successfully");
        let input_share = input_manager::launch_input_listener(
            input_socket,
            uinput_share.uinput_sender().clone(),
        )
        .expect("uinput manager shoudl launch successfully");

        // Spin up workers.
        let handles = Self::launch_workers(worker_sockets, uinput_share.uinput_sender());

        // Listen for parent death.
        thread::spawn(|| {
            let mut buf = [0; 256];
            let stdin = &std::io::stdin();
            let mes_res: postcard::Result<(Vec<u8>, _)> = postcard::from_io((stdin, &mut buf));
            match mes_res {
                Ok(_) => println!("Received unexpected message from stdin"),
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
        });

        // If stdin dies, it means the parent died, so just exit.

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
        worker_sockets: Vec<UnixStream>,
        uinput_sender: &Sender<message::MsgToUInput>,
    ) -> Vec<JoinHandle<()>> {
        let mut handlers = Vec::new();
        for (i, socket) in worker_sockets.into_iter().enumerate() {
            let uinput_sender = uinput_sender.clone();
            let handle = thread::spawn(move || {
                loop {
                    let mut buf = [0; 256];
                    let mes_res = postcard::from_io((&socket, &mut buf));
                    let mes: message::MsgToWorker = match mes_res {
                        Ok(v) => v.0,
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

    fn get_sockets(nb_sockets: u16) -> std::io::Result<(UnixStream, Vec<UnixStream>)> {
        let mut sockets = Vec::new();
        // Use +1 here to access input socket.
        for _ in 0..(nb_sockets + 1) {
            let mut payload = [0u8];
            let mut iov = [IoSliceMut::new(&mut payload)];
            let mut cmsg_buffer = nix::cmsg_space!([std::os::unix::io::RawFd; 1]);
            let msg = recvmsg::<()>(
                std::io::stdin().as_raw_fd(),
                &mut iov,
                Some(&mut cmsg_buffer),
                MsgFlags::empty(),
            )?;
            for cmsg in msg.cmsgs()? {
                if let ControlMessageOwned::ScmRights(fds) = cmsg {
                    for fd in fds {
                        unsafe {
                            sockets.push(UnixStream::from_raw_fd(fd));
                        }
                    }
                }
            }
        }
        Ok((
            sockets.pop().expect("there should be at least one socket"),
            sockets,
        ))
    }
}
fn main() {
    PrivHandler::launch_handler();
}
