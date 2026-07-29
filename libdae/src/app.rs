//! Handles the creation of the necessary threads and processes.
use std::{
    collections::HashSet,
    io::IoSlice,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::net::UnixStream,
    },
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
use serde::{Deserialize, Serialize};

use crate::{
    api::{Api, ApiHolder},
    binder::Binder,
    input::Keybind,
    wayland_int::display_output::{self, Screen, ScreenInfo, ScreenSpace},
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

    // Get info about the screen outputs.
    let ex = "displays should have fetchable information";
    // TODO: Don't panic when no size, just remove absolute cursor feature.
    let list_outputs = display_output::get_list_outputs().expect(ex);
    // TODO: Use screens to create shell layers.
    let screens: Vec<Screen> = list_outputs.get_screens().expect(ex);
    let screen_space = display_output::ScreenSpace::from_monitors(
        &screens
            .iter()
            .map(|v| v.screen_info().clone())
            .collect::<Vec<ScreenInfo>>(),
    );
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
    let (input_socket, closure_sockets) = share_sockets(&socket_core_end, binder.max_threads())
        .expect("sockets should be created successfully");
    let api_instances: Arc<Mutex<Vec<Api>>> = Arc::new(Mutex::new(Vec::new()));
    for socket in closure_sockets {
        api_instances
            .lock()
            .expect("mutex should lock")
            .push(Api::new(socket));
    }

    // Send the bindings over.
    let mut keybinds = HashSet::new();
    for binding in binder.bindings() {
        keybinds.insert(binding.0.clone());
    }
    keybinds.insert(binder.toggle_bindings_key());
    keybinds.insert(binder.exit_key());
    postcard::to_io(&keybinds, &input_socket).expect("postcard should be able to serialize");

    // Start thread pool.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(binder.max_threads() as usize)
        .build()
        .expect("thread pool should have been initialized");
    // Listen to input.
    loop {
        let mut buf = [0; 256];
        let key_event_res = postcard::from_io((&input_socket, &mut buf));
        let key_event: Keybind = match key_event_res {
            Ok(v) => v.0,
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
                    let mut keybinds = HashSet::new();
                    keybinds.insert(binder.toggle_bindings_key());
                    keybinds.insert(binder.exit_key());
                    postcard::to_io(&keybinds, &input_socket)
                        .expect("postcard should be able to serialize");
                } else {
                    postcard::to_io(&keybinds, &input_socket)
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
) -> std::io::Result<(UnixStream, Vec<UnixStream>)> {
    let mut sockets = Vec::new();
    // Use +1 here to create input socket.
    for s in 0..(nb_worker_sockets + 1) {
        let (socket_core, socket_child) = std::os::unix::net::UnixStream::pair()?;
        let socket_child_fd: std::os::fd::RawFd = socket_child.as_raw_fd();
        // Send different payload on input socket.
        let payload = if s == nb_worker_sockets { [1u8] } else { [0u8] };
        let iov = [IoSlice::new(&payload)];
        let cmsg = [ControlMessage::ScmRights(&[socket_child_fd])];
        sendmsg::<()>(
            child_stdin.as_raw_fd(),
            &iov,
            &cmsg,
            MsgFlags::empty(),
            None,
        )?;
        sockets.push(socket_core);
    }
    Ok((
        sockets.pop().expect("there should be at least one socket"),
        sockets,
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
