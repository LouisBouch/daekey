//! Logic for starting the daemon.

use std::{
    any::Any,
    error::Error,
    fmt::Display,
    thread::{self, JoinHandle},
};

use crossbeam_channel::Receiver;

use crate::{
    client_gateway::Gateway,
    device_interface::{input::Input, uinput::UInput},
    ipc::connector::{self, SocketServerError},
    watcher::{ThreadExit, ThreadStatus, Watcher},
};

/// Different types of threads launched by the daemon.
#[derive(Debug, Clone)]
pub enum Thread {
    /// The router thread.
    ClientGateway,
    /// The input device interface thread.
    Input,
    /// The uinput device interface thread.
    Uinput,
    /// The socket server thread..
    SocketServer,
}
impl Display for Thread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Thread::ClientGateway => {
                write!(f, "router")
            }
            Thread::Input => {
                write!(f, "input interface")
            }
            Thread::Uinput => {
                write!(f, "uinput interface")
            }
            Thread::SocketServer => {
                write!(f, "socket server")
            }
        }
    }
}
/// Error during the daemon's runtime.
#[derive(Debug)]
pub enum RuntimeError {
    /// Panic during the execution of a thread.
    Thread(ThreadError),
    /// Error when initializing the socket server.
    SocketServer(SocketServerError),
}
impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Thread(_) => {
                write!(f, "a thread quit when it should not have")
            }
            RuntimeError::SocketServer(_) => {
                write!(
                    f,
                    "failed to create the socket server, restarting the service might help"
                )
            }
        }
    }
}
impl Error for RuntimeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RuntimeError::Thread(thread_error) => Some(thread_error),
            RuntimeError::SocketServer(socket_server_error) => Some(socket_server_error),
        }
    }
}

// TODO: NEXT FIX CORE AND ALL THE FUCKING ERRORS
/// Launches the daemon.
///
/// # Return
///
/// This function should run forever, so it will only return on an error.
pub fn launch_daemon() -> RuntimeError {
    // Start socket server.
    let (fr_connector_tx, fr_connector_rx) = crossbeam_channel::unbounded();
    let socket_server = match connector::create_socket_server() {
        Ok(s) => s,
        Err(e) => {
            return RuntimeError::SocketServer(e);
        }
    };
    let socket_server_handle = connector::listen_socket_server(socket_server, fr_connector_tx);

    // Define channels.
    let (fr_dev_interface_tx, fr_dev_interface_rx) = crossbeam_channel::unbounded();
    let (to_uinput_tx, to_uinput_rx) = crossbeam_channel::unbounded();
    let (to_input_tx, to_input_rx) = crossbeam_channel::unbounded();

    // Start UInput interface.
    // TODO: NEXT Convert to logical device.
    let uinput_handle = UInput::new(fr_dev_interface_tx.clone(), to_uinput_rx).launch();

    // Start Input interface.
    // TODO: NEXT Make input send its stuff to the logical device instead of the gateway.
    let input_handle = Input::new(fr_dev_interface_tx, to_input_rx, to_uinput_tx.clone()).launch();

    // Start Router.
    let gateway_handle = Gateway::new(
        fr_dev_interface_rx,
        fr_connector_rx,
        to_uinput_tx,
        to_input_tx,
    )
    .launch();
    let handles = vec![
        (socket_server_handle, Thread::SocketServer),
        (uinput_handle, Thread::Uinput),
        (input_handle, Thread::Input),
        (gateway_handle, Thread::ClientGateway),
    ];
    let (watcher, rx) = Watcher::new();
    for (handle, thread) in handles {
        watcher.watch_thread(thread, handle);
    }
    let ThreadExit {
        key: thread,
        status,
    } = rx.recv().expect("watcher should be alive");
    let e = match status {
        ThreadStatus::Returned(_) => ThreadError::UnexpectedExit { thread },
        ThreadStatus::Panicked(e) => ThreadError::Panic {
            thread,
            formatted_panic: e,
        },
    };
    RuntimeError::Thread(e)
}

/// Error while running a thread.
#[derive(Debug)]
pub enum ThreadError {
    /// The thread panicked.
    Panic {
        thread: Thread,
        formatted_panic: String,
    },
    /// The thread exited unexpectedly. (This should be impossible, the thread loop only exits on panics)
    UnexpectedExit { thread: Thread },
}
impl Display for ThreadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadError::Panic {
                thread,
                formatted_panic,
            } => {
                write!(f, "{thread} panicked: {formatted_panic}")
            }
            ThreadError::UnexpectedExit { thread } => {
                write!(f, "{thread} exited unexpectedly")
            }
        }
    }
}
impl Error for ThreadError {}
