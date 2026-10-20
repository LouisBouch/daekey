//! Logic for starting the daemon.

use std::{
    any::Any,
    error::Error,
    fmt::Display,
    thread::{self, JoinHandle},
};

use crate::{
    device_interface::{input::Input, uinput::UInput},
    ipc::connector::{self, SocketServerError},
    router::Router,
};

/// Different types of threads launched by the daemon.
#[derive(Debug, Clone)]
pub enum Thread {
    /// The router thread.
    Router,
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
            Thread::Router => {
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
    let uinput_handle = UInput::new(fr_dev_interface_tx.clone(), to_uinput_rx).launch();

    // Start Input interface.
    let input_handle = Input::new(fr_dev_interface_tx, to_input_rx, to_uinput_tx.clone()).launch();

    // Start Router.
    let router_handle = Router::new(
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
        (router_handle, Thread::Router),
    ];
    // Should never return.
    RuntimeError::Thread(watch_handles(handles))
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
/// Checks the handles and return if any of them fails.
fn watch_handles(handle_tuples: Vec<(JoinHandle<()>, Thread)>) -> ThreadError {
    let (err_tx, err_rx) = crossbeam_channel::unbounded();
    for (handle, thread) in handle_tuples {
        let err_tx = err_tx.clone();
        thread::spawn(move || match handle.join() {
            Ok(_) => err_tx.send(ThreadError::UnexpectedExit { thread }),
            Err(e) => err_tx.send(ThreadError::Panic {
                thread: thread,
                formatted_panic: format_panic(e.as_ref()),
            }),
        });
    }
    err_rx.recv().expect("senders should not be closed")
}
/// Turns a panic payload from a join on a handle into a readable [`String`].
fn format_panic(panic: &(dyn Any + Send + 'static)) -> String {
    if let Some(e) = panic.downcast_ref::<String>() {
        e.to_owned()
    } else if let Some(&e) = panic.downcast_ref::<&str>() {
        e.to_owned()
    } else {
        "unknown panic type".to_owned()
    }
}
