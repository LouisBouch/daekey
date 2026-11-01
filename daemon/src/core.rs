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
    logical_device::router::{LogicalDevice, LogicalDeviceHandle},
    watcher::{ThreadExit, ThreadStatus, Watcher},
};

/// Different types of threads launched by the daemon.
#[derive(Debug, Clone)]
pub enum Thread {
    /// The router thread.
    ClientGateway,
    /// The input device interface thread.
    Input,
    /// The [`LogicalDevice`] thread.
    LogicalDevice,
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
            Thread::LogicalDevice => {
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
    // Define channels.
    let (connector_to_gateway_tx, connector_to_gateway_rx) = crossbeam_channel::unbounded();
    let (logicaldevice_to_gateway_tx, logicaldevice_to_gateway_rx) = crossbeam_channel::unbounded();

    // Start socket server.
    let socket_server = match connector::create_socket_server() {
        Ok(s) => s,
        Err(e) => {
            return RuntimeError::SocketServer(e);
        }
    };
    let socket_server_handle =
        connector::listen_socket_server(socket_server, connector_to_gateway_tx);

    // Start UInput interface.
    let LogicalDeviceHandle {
        handle: logical_device_handle,
        simulated_tx_maker,
        physical_tx_maker,
    } = LogicalDevice::new(logicaldevice_to_gateway_tx).launch();

    // Start Input interface.
    let input_handle = Input::new(physical_tx_maker).launch();

    // Start Router.
    let gateway_handle = Gateway::new(
        logicaldevice_to_gateway_rx,
        connector_to_gateway_rx,
        simulated_tx_maker,
    )
    .launch();
    let handles = vec![
        (socket_server_handle, Thread::SocketServer),
        (logical_device_handle, Thread::LogicalDevice),
        (input_handle, Thread::Input),
        (gateway_handle, Thread::ClientGateway),
    ];
    // Wait for a thread to exit. If it does, error out, the threads should run forever.
    let watcher = Watcher::new();
    for (handle, thread) in handles {
        watcher.watch_thread(thread, handle);
    }
    let ThreadExit {
        key: thread,
        status,
    } = watcher
        .rx()
        .recv()
        .expect("watcher should be alive until the end of the scope");
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
