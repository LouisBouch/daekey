//! The stateful client that holds mappings and handles the connection to the daemon.

use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    io,
    sync::{Arc, Mutex},
};

use common_lib::ipc::{
    channel::{
        rx::Rx,
        tx::{SendError, SerializeError, Tx},
    },
    message::{FromClient, FromDaemon},
};

use crate::{
    action::Action,
    connection::{ConnectionError, socket_from_daemon},
    daemon_handle::DaemonHandle,
};
use crate::{
    builder::{BindingsBuildError, BindingsBuilder, PreparedBindings},
    thread_pool::ThreadPoolBuildError,
};

#[derive(Debug)]
/// Possible errors when instantiating a new [`Client`].
pub enum InstantiationError {
    /// Failed to initiate the thread pool.
    ThreadPool(ThreadPoolBuildError),
    /// Failed to connect to the daemon.
    DaemonConnection(ConnectionError),
    /// Failed to duplicate the daemon's socket.
    SocketCloning(io::Error),
    /// Failed to send bindings.
    BindingSend(SendError),
    /// Failed to serialize binings.
    BindingSerialization(SerializeError),
}
impl Display for InstantiationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstantiationError::ThreadPool(_) => {
                write!(f, "failed to create the thread pools")
            }
            InstantiationError::DaemonConnection(_) => {
                write!(f, "failed to connect to the daemon")
            }
            InstantiationError::SocketCloning(_) => {
                write!(f, "failed to clone the daemon's socket")
            }
            InstantiationError::BindingSend(_) => {
                write!(f, "failed to send the bindings to the daemon")
            }
            InstantiationError::BindingSerialization(_) => {
                write!(
                    f,
                    "failed to serialize the bindings before sending them over the socket"
                )
            }
        }
    }
}
impl Error for InstantiationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InstantiationError::ThreadPool(thread_pool_build_error) => {
                Some(thread_pool_build_error)
            }
            InstantiationError::DaemonConnection(connection_error) => Some(connection_error),
            InstantiationError::SocketCloning(error) => Some(error),
            InstantiationError::BindingSend(send_error) => Some(send_error),
            InstantiationError::BindingSerialization(ser_error) => Some(ser_error),
        }
    }
}
#[derive(Debug)]
/// Possible errors when running the [`Client`].
pub enum RunError {
    /// Socket to the daemon was disconnected.
    SocketConnection,
}
impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::SocketConnection => {
                write!(f, "the daemon's socket closed unexpectedly")
            }
        }
    }
}
impl Error for RunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RunError::SocketConnection => None,
        }
    }
}

/// An instance of a connection to the daemon.
pub struct Client {
    /// The connection that will receive messages from the daemon.
    daemon_rx: Rx<FromDaemon>,
    /// The list of [`Action`] the client can execute.
    actions: Vec<Action>,
}
impl Client {
    /// Creates a new [`Client`] and connects to the daemon.
    ///
    /// # Aguments
    ///
    /// * `bindings` - Prepared bindings with associated actions.
    pub fn connect(bindings: PreparedBindings) -> Result<Self, InstantiationError> {
        let daemon_socket = socket_from_daemon().map_err(InstantiationError::DaemonConnection)?;
        let daemon_rx = Rx::new(
            daemon_socket
                .try_clone()
                .map_err(InstantiationError::SocketCloning)?,
        );

        let mut daemon_tx = Tx::new(daemon_socket);
        let ctx_maker = || DaemonHandle::new(daemon_tx.clone());

        let (bindings, prepared_actions) = bindings.deconstruct();
        let actions = prepared_actions
            .into_actions(ctx_maker)
            .map_err(InstantiationError::ThreadPool)?;

        daemon_tx
            .send(&FromClient::SetBindings(bindings))
            .map_err(InstantiationError::BindingSend)?
            .map_err(InstantiationError::BindingSerialization)?;

        Ok(Self { daemon_rx, actions })
    }
    /// Start the client.
    pub fn run(&mut self) -> Result<(), RunError> {
        loop {
            match self.daemon_rx.recv() {
                // TODO: NEXT add message types and handle error from receiver. Improve [`RunError`].
                Ok(m) => match m {
                    Ok(_) => todo!(),
                    Err(_) => todo!(),
                },
                Err(e) => {
                    // For a regular socket, DeserializeUnexpectedEnd represents disconnect.

                    // if e == postcard::Error::DeserializeUnexpectedEnd {
                    //     return Err(RunError::SocketConnection);
                    // }
                    // let ctx = "failed to receive message from the daemon, skipping";
                    // print_err(&ErrorContext::new(ctx.into(), &e));
                }
            }
        }
    }
}
// TODO: Impl remaining functions.
