//! The app setup goes through the binder.
use std::{
    collections::{HashMap, HashSet},
    io::IoSlice,
    os::{
        fd::{AsRawFd, FromRawFd, IntoRawFd},
        unix::net::UnixStream,
    },
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use evdev::KeyCode;
use nix::sys::socket::{ControlMessage, MsgFlags, sendmsg};
use serde::{Deserialize, Serialize};

use crate::{
    api::Api,
    keys::{KeyState, Keybind},
    modifiers,
};

/// Holds everything necessary for the app to work.
pub struct Binder {
    /// Maximum number of threads to run closures with.
    max_threads: u16,
    /// The closures for each keybindings.
    bindings: HashMap<Keybind, Arc<dyn Fn(&Api) + Send + Sync>>,
    /// Keybind that toggles the other keybindings.
    toggle_bindings_key: Keybind,
    /// Whether the keybinds are paused or not.
    paused: bool,
}
impl Binder {
    pub fn new(max_threads: u16) -> Self {
        let toggle_bindings_key =
            Keybind::new(KeyCode::KEY_PAUSE, KeyState::Pressed, modifiers::NONE);
        Binder {
            max_threads,
            bindings: HashMap::new(),
            toggle_bindings_key,
            paused: false,
        }
    }
    /// Create new keybinding.
    ///
    /// # Arguments
    ///
    /// * `key_event` - The key to bind it to.
    /// * `closure` - The closure that will run when the binding is activated.
    pub fn create_binding<F>(&mut self, key_event: Keybind, closure: F)
    where
        F: Fn(&Api) + 'static + Send + Sync,
    {
        self.bindings.insert(key_event, Arc::new(closure));
    }
    /// Defines a key to turn keybind activation on or off.
    pub fn set_toggle_bindings_key(&mut self, key: Keybind) {
        self.toggle_bindings_key = key;
    }

    /// Start the app. It creates the necessary threads and processes.
    pub fn launch(mut self) {
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
        let (socket_core_end, socket_priv_end) = std::os::unix::net::UnixStream::pair().unwrap();
        let fd_socket_priv_end = socket_priv_end.into_raw_fd();

        // Launch privileged process with necessary permissions.
        let mut child = Command::new("sudo")
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
            .expect("command should not error out");

        // Notify the privileged process of the context.
        let context = SetupContext {
            nb_threads: self.max_threads,
        };
        postcard::to_io(&context, &socket_core_end).expect("postcard should be able to serialize");
        // Wait for context acknowledgement from the privileged process, otherwise the ancillary data
        // from socket creation will get tacked on to the last message sent.
        let _ack: bool = postcard::from_io((&socket_core_end, &mut [0; 256]))
            .expect("priv process sohuld ack when it received context")
            .0;

        // Create sockets and send them over to the child.
        let (input_socket, closure_sockets) =
            Self::share_sockets(&socket_core_end, self.max_threads)
                .expect("sockets should be created successfully");
        let shared_closure_sockets = Arc::new(Mutex::new(closure_sockets));

        // Send the bindings over.
        let mut keybinds = HashSet::new();
        for binding in &self.bindings {
            keybinds.insert(binding.0.clone());
        }
        keybinds.insert(self.toggle_bindings_key);
        postcard::to_io(&keybinds, &input_socket).expect("postcard should be able to serialize");

        // Start thread pool.
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.max_threads as usize)
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
            let Some(closure) = self.bindings.get(&key_event).cloned() else {
                if key_event == self.toggle_bindings_key {
                    self.paused = !self.paused;
                    if self.paused {
                        let mut keybind = HashSet::new();
                        keybind.insert(self.toggle_bindings_key);
                        postcard::to_io(&keybind, &input_socket)
                            .expect("postcard should be able to serialize");
                    } else {
                        postcard::to_io(&keybinds, &input_socket)
                            .expect("postcard should be able to serialize");
                    }
                    continue;
                }
                eprintln!("key received from input is not bound: '{key_event:?}'");
                break;
            };

            let socket_r = {
                let mut g = shared_closure_sockets.lock().expect("should yield lock");
                g.pop()
            };
            match socket_r {
                Some(s) => {
                    let b = Api::new(s, shared_closure_sockets.clone());
                    pool.spawn(move || closure(&b));
                }
                None => println!("Not enough sockets, skipping key..."),
            }
        }
        child.wait().unwrap();
    }
    // Create and share sockets that thte privileged process will use.
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
}

#[doc(hidden)]
#[derive(Serialize, Deserialize, Debug)]
/// Context for the privileged process.
pub struct SetupContext {
    /// Numebr of threads to deploy
    nb_threads: u16,
}
impl SetupContext {
    pub fn nb_threads(&self) -> u16 {
        self.nb_threads
    }
}
