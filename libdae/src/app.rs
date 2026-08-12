//! Handles the creation of the necessary threads and processes.
use std::{
    collections::HashSet,
    error::Error,
    fmt::Display,
    io::IoSlice,
    marker::PhantomData,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd, RawFd},
        unix::net::UnixStream,
    },
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
};

use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    api::{Api, ApiHolder},
    binder::Binder,
    compositor_interface::{self, CompositorInterface, ScreenSpace},
    input::Keybind,
    message::{MsgToInput, MsgToUInput, MsgToWorker},
};

/// Start the privileged process.
///
/// # Arguments
///
/// * `socket_priv_end` - Socket that will be used as stdin for the privileged process.
fn start_priv_process(socket_priv_end: UnixStream) -> Child {
    let cur_bin_path = std::env::current_exe().unwrap();
    let req_bin_dir = cur_bin_path.parent().unwrap().parent().unwrap();
    let priv_handler_bin_path = req_bin_dir.join("priv_handler");

    let uid = std::env::var("USER").expect("user id should be fetchable");

    // Allow new group to access uinput.
    let ex = "command should run successfully";
    Command::new("sudo")
        .args(["groupadd", "-f", "uinput"])
        .status()
        .expect(ex);
    Command::new("sudo")
        .args(["chgrp", "uinput", "/dev/uinput"])
        .status()
        .expect(ex);
    Command::new("sudo")
        .args(["chmod", "660", "/dev/uinput"])
        .status()
        .expect(ex);

    let fd_socket_priv_end = socket_priv_end.into_raw_fd();

    // Launch privileged process with necessary permissions.
    Command::new("sudo")
        .args([
            "setpriv",
            "--groups",
            "input,uinput",
            "--ruid",
            &uid,
            "--rgid",
            &uid,
            priv_handler_bin_path
                .to_str()
                .expect("path should be valid"),
        ])
        .stdin(unsafe { Stdio::from_raw_fd(fd_socket_priv_end) })
        .spawn()
        .expect("command should not error out")
}

/// Entry point into the app.
/// Starts the necessary process and threads.
pub fn launch(mut binder: Binder) {
    let (socket_core_end, socket_priv_end) = std::os::unix::net::UnixStream::pair().unwrap();
    let mut child = start_priv_process(socket_priv_end);

    // TODO: Use upd_rec to listen for udpates from compositor.
    let (cmp_intf, upd_rec) = CompositorInterface::init();
    //TODO: if no screen info, just disable some functionalities.
    let screen_info = cmp_intf
        .req_screen_info()
        .expect("screen info should be available");
    let screen_space = compositor_interface::ScreenSpace::from_monitors(&screen_info);

    // Notify the privileged process of the context.
    let context = SetupContext {
        nb_threads: binder.max_threads(),
        screen_space: screen_space.clone(),
    };
    postcard::to_io(&context, &socket_core_end).expect("postcard should be able to serialize");
    // Wait for context acknowledgement from the privileged process, otherwise the ancillary data
    // from socket creation will get tacked on to the last message sent.
    let _ack: bool = postcard::from_io((&socket_core_end, &mut [0; 256]))
        .expect("priv process should have acked when it received context")
        .0;

    // Create sockets and send them over to the child.
    let mut input_socket = None;
    let mut uinput_socket = None;
    let mut worker_sockets = Vec::new();
    let sockets = share_sockets(&socket_core_end, binder.max_threads())
        .expect("sockets should be created and sent successfully");
    for socket in sockets {
        match socket {
            WrappedSocketBag::WorkerCore(wrapped_socket) => worker_sockets.push(wrapped_socket),
            WrappedSocketBag::InputCore(wrapped_socket) => input_socket = Some(wrapped_socket),
            WrappedSocketBag::UInputCore(wrapped_socket) => uinput_socket = Some(wrapped_socket),
            _ => eprintln!("There should not be any child socket in the core: {socket:?}"),
        }
    }
    let (mut input_socket, uinput_socket) = (
        input_socket.expect("input_socket should be initialized"),
        uinput_socket.expect("uinput_socket should be initialized"),
    );
    let api_instances: Arc<Mutex<Vec<Api>>> = Arc::new(Mutex::new(Vec::new()));
    for socket in worker_sockets {
        api_instances
            .lock()
            .expect("mutex should lock")
            .push(Api::new(socket, cmp_intf.clone()));
    }

    // Send the bindings over.
    let mut keybinds = HashSet::new();
    for binding in binder.bindings() {
        keybinds.insert(binding.0.clone());
    }
    keybinds.insert(binder.toggle_bindings_key());
    keybinds.insert(binder.exit_key());
    input_socket
        .send(&MsgToInput::ChangeBindings(keybinds.clone()))
        .expect("postcard should be able to serialize");

    // Start thread pool.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(binder.max_threads() as usize)
        .build()
        .expect("thread pool should have been initialized");
    // Listen to input.
    thread::spawn(move || {
        loop {
            let key_event_res = input_socket.receive();
            let key_event: Keybind = match key_event_res {
                Ok(v) => v,
                Err(e) => match e {
                    postcard::Error::DeserializeUnexpectedEnd => {
                        eprintln!("child process died, aborting: '{e}'");
                        std::process::exit(1);
                    }
                    _ => {
                        eprintln!("unexpected error, could not read from socket, aborting: '{e}'");
                        std::process::exit(1);
                    }
                },
            };
            let Some(closure) = binder.bindings().get(&key_event).cloned() else {
                if key_event == binder.toggle_bindings_key() {
                    binder.set_paused(!binder.paused());
                    if binder.paused() {
                        let mut new_keybinds = HashSet::new();
                        new_keybinds.insert(binder.toggle_bindings_key());
                        new_keybinds.insert(binder.exit_key());
                        input_socket
                            .send(&MsgToInput::ChangeBindings(new_keybinds))
                            .expect("postcard should be able to serialize");
                    } else {
                        input_socket
                            .send(&MsgToInput::ChangeBindings(keybinds.clone()))
                            .expect("postcard should be able to serialize");
                    }
                    continue;
                } else if key_event == binder.exit_key() {
                    println!("Process terminated by user");
                    // TODO: Exit more gracefully.
                    std::process::exit(0);
                }
                eprintln!("key received from input is not bound: '{key_event:?}'");
                break;
            };
            // Spawn closure with an [`ApiHolder`].
            match api_instances.lock().expect("should yield lock").pop() {
                Some(api) => {
                    let api_holder = ApiHolder::new(api, api_instances.clone());
                    pool.spawn(move || closure(&api_holder));
                }
                None => println!("Not enough sockets/threads, skipping key..."),
            }
        }
    });
    child.wait().unwrap();
}
/// Create and share sockets that the privileged process will use.
///
/// # Arguments
///
/// * `child_stdin` - When to send the created sockets to.
/// * `nb_worker_sockets` - Number of sockets to send over to the privileged process.
fn share_sockets(
    child_stdin: &UnixStream,
    nb_worker_sockets: u16,
) -> std::io::Result<Vec<WrappedSocketBag>> {
    let mut sockets = Vec::new();
    let buffer_size = 256;
    for _ in 0..(nb_worker_sockets) {
        sockets.push(send_socket(
            &child_stdin,
            SocketType::WorkerCore,
            SocketType::WorkerPriv,
            buffer_size,
        )?);
    }
    sockets.push(send_socket(
        &child_stdin,
        SocketType::InputCore,
        SocketType::InputPriv,
        buffer_size,
    )?);
    sockets.push(send_socket(
        &child_stdin,
        SocketType::UInputCore,
        SocketType::UInputPriv,
        buffer_size,
    )?);
    Ok(sockets)
}
/// Send a socket over another socket.
/// # Parameters
///
/// * `channel_socket` - The socket over which the new socket will be sent over.
/// * `socket_type` - The type of socket being sent over. I.e, what it will be used for.
///
/// # Return
///
/// The end of the socket that can be used to communicate with the sent socket
///
fn send_socket(
    channel_socket: &UnixStream,
    kept_socket_type: SocketType,
    sent_socket_type: SocketType,
    buffer_size: usize,
) -> std::io::Result<WrappedSocketBag> {
    let (socket_to_keep, socket_to_send) = std::os::unix::net::UnixStream::pair()?;
    let socket_child_fd: std::os::fd::RawFd = socket_to_send.as_raw_fd();
    // Send different payload on input socket.
    let payload = [sent_socket_type as u8];
    let iov = [IoSlice::new(&payload)];
    let cmsg = [ControlMessage::ScmRights(&[socket_child_fd])];
    sendmsg::<()>(
        channel_socket.as_raw_fd(),
        &iov,
        &cmsg,
        MsgFlags::empty(),
        None,
    )?;
    Ok(WrappedSocketBag::from_socket_type(
        socket_to_keep,
        kept_socket_type,
        buffer_size,
    ))
}
#[doc(hidden)]
#[derive(Serialize, Deserialize, Debug)]
/// Context for the privileged process.
pub struct SetupContext {
    /// Number of threads to deploy
    nb_threads: u16,
    /// Information about the monitor layout.
    screen_space: ScreenSpace,
}
impl SetupContext {
    pub fn nb_threads(&self) -> u16 {
        self.nb_threads
    }
    pub fn screen_space(&self) -> &ScreenSpace {
        &self.screen_space
    }
}
// TODO: Make a macro to write enums and from_socket_type automatically from a single enum
// definition.
// TODO: Figure out what should be doc hidden here.
#[doc(hidden)]
/// A wrapped [`UnixStream`] to ensure only one type of data can be sent and another received.
#[derive(Debug)]
pub struct WrappedSocket<S: Serialize, R: DeserializeOwned> {
    /// The socket the communication will be done over.
    socket: UnixStream,
    /// Buffer for the receiver.
    buffer: Vec<u8>,
    /// The struct is required to hold the struct somewhere, so store them here.
    _t: PhantomData<(S, R)>,
}

impl<S: Serialize, R: DeserializeOwned> WrappedSocket<S, R> {
    pub fn new(socket: UnixStream, buffer_size: usize) -> Self {
        WrappedSocket {
            socket,
            buffer: vec![0u8; buffer_size],
            _t: PhantomData,
        }
    }
    /// Send a message through the socket.
    pub fn send(&self, msg: &S) -> postcard::Result<()> {
        postcard::to_io(&msg, &self.socket)?;
        Ok(())
    }
    /// Waits for a message from the other end of the socket.
    pub fn receive(&mut self) -> postcard::Result<R> {
        Ok(postcard::from_io((&self.socket, &mut self.buffer))?.0)
    }
    // Extract the raw file descriptor.
    pub fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}
#[doc(hidden)]
#[repr(i8)]
#[derive(Debug, Clone, Copy)]
/// Describes what a socket will be used for,
pub enum SocketType {
    /// A worker will own the other side of this socket.
    WorkerCore = 1,
    /// The Input thread will own the other side of this this socket.
    InputCore = 2,
    /// The UInput thread will own the other side of this this socket.
    UInputCore = 3,

    /// A worker will own this socket.
    WorkerPriv = -1,
    /// The Input thread will own this socket.
    InputPriv = -2,
    /// The UInput thread will own this socket.
    UInputPriv = -3,
}
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct InvalidSocketTypeId(i8);
impl Error for InvalidSocketTypeId {}
impl Display for InvalidSocketTypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Invalid socket type id: {}", self.0)
    }
}
impl TryFrom<i8> for SocketType {
    type Error = InvalidSocketTypeId;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SocketType::WorkerCore),
            2 => Ok(SocketType::InputCore),
            3 => Ok(SocketType::UInputCore),
            -1 => Ok(SocketType::WorkerPriv),
            -2 => Ok(SocketType::InputPriv),
            -3 => Ok(SocketType::UInputPriv),
            other => Err(InvalidSocketTypeId(other)),
        }
    }
}

pub type WorkerCoreSocket = WrappedSocket<MsgToWorker, ()>;
pub type InputCoreSocket = WrappedSocket<MsgToInput, Keybind>;
pub type UInputCoreSocket = WrappedSocket<MsgToUInput, ()>;

pub type WorkerPrivSocket = WrappedSocket<(), MsgToWorker>;
pub type InputPrivSocket = WrappedSocket<Keybind, MsgToInput>;
pub type UInputPrivSocket = WrappedSocket<(), MsgToUInput>;

/// Define the possible wrapped sockets given the ['SocketType']s.
#[derive(Debug)]
pub enum WrappedSocketBag {
    WorkerCore(WorkerCoreSocket),
    InputCore(InputCoreSocket),
    UInputCore(UInputCoreSocket),

    WorkerPriv(WorkerPrivSocket),
    InputPriv(InputPrivSocket),
    UInputPriv(UInputPrivSocket),
}
impl WrappedSocketBag {
    /// Automatically create a wrapped socket from a [`SocketType`].
    pub fn from_socket_type(
        socket: UnixStream,
        socket_type: SocketType,
        buffer_size: usize,
    ) -> WrappedSocketBag {
        match socket_type {
            SocketType::WorkerCore => {
                WrappedSocketBag::WorkerCore(WorkerCoreSocket::new(socket, buffer_size))
            }
            SocketType::InputCore => {
                WrappedSocketBag::InputCore(InputCoreSocket::new(socket, buffer_size))
            }
            SocketType::UInputCore => {
                WrappedSocketBag::UInputCore(UInputCoreSocket::new(socket, buffer_size))
            }
            SocketType::WorkerPriv => {
                WrappedSocketBag::WorkerPriv(WorkerPrivSocket::new(socket, buffer_size))
            }
            SocketType::InputPriv => {
                WrappedSocketBag::InputPriv(InputPrivSocket::new(socket, buffer_size))
            }
            SocketType::UInputPriv => {
                WrappedSocketBag::UInputPriv(UInputPrivSocket::new(socket, buffer_size))
            }
        }
    }
}
