//! Logic for starting the daemon.

use std::{any::Any, error::Error, fmt::Display};

use crate::{
    device_interface::{input::Input, uinput::UInput},
    ipc::connector::{self, SocketServerError},
    router::Router,
};

/// Different types of threads launched by the daemon.
#[derive(Debug)]
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
    /// Error during the execution of a thread.
    ///
    /// # Arguments
    ///
    /// * `String` - The string extracted from the panic message.
    /// * `Thread` - The Thread that panicked.
    Thread(String, Thread),
    /// Error when initializing the socket server.
    SocketServer(SocketServerError),
}
impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::Thread(panic, thread) => {
                write!(f, "{thread} panicked: {panic}")
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
            RuntimeError::Thread(_, _) => None,
            RuntimeError::SocketServer(socket_server_error) => Some(socket_server_error),
        }
    }
}

/// Launches the daemon.
pub fn launch_daemon() -> Result<(), RuntimeError> {
    // Start socket server.
    let (connector_tx, fr_connector_rx) = crossbeam_channel::unbounded();
    let socket_server = match connector::create_socket_server() {
        Ok(s) => s,
        Err(e) => {
            return Err(RuntimeError::SocketServer(e));
        }
    };
    let socket_server_handle = connector::listen_socket_server(socket_server, connector_tx);

    // Define channels.
    let (fr_dev_interface_tx, fr_dev_interface_rx) = crossbeam_channel::unbounded();
    let (fr_connection_tx, fr_connection_rx) = crossbeam_channel::unbounded();
    let (to_uinput_tx, to_uinput_rx) = crossbeam_channel::unbounded();
    let (to_input_tx, to_input_rx) = crossbeam_channel::unbounded();

    // Start UInput interface.
    let uinput_handle = UInput::new(fr_dev_interface_tx.clone(), to_uinput_rx).launch();

    // Start Input interface.
    let input_handle = Input::new(
        fr_dev_interface_tx.clone(),
        to_input_rx,
        to_uinput_tx.clone(),
    )
    .launch();

    // Start Router.
    let router_handle = Router::new(
        fr_dev_interface_rx,
        fr_connection_rx,
        fr_connector_rx,
        to_uinput_tx,
        to_input_tx,
        fr_connection_tx,
    )
    .launch();

    // Join on created threads.
    if let Err(e) = socket_server_handle.join() {
        return Err(RuntimeError::Thread(format_panic(&e), Thread::SocketServer));
    }
    if let Err(e) = uinput_handle.join() {
        return Err(RuntimeError::Thread(format_panic(&e), Thread::Uinput));
    }
    if let Err(e) = input_handle.join() {
        return Err(RuntimeError::Thread(format_panic(&e), Thread::Input));
    }
    if let Err(e) = router_handle.join() {
        return Err(RuntimeError::Thread(format_panic(&e), Thread::Router));
    }
    Ok(())
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
