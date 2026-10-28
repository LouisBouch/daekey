//! The hub of communication. Routes messages from uinpu/input to the necessary connected process.
//! Also handles the creation of the client connections in order to keep the routing list up to date.

use crossbeam_channel::{Receiver, Sender};
use library::ipc::{message, socket_connection::ConnectionRx, socket_connection::ConnectionTx};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::os::unix::net::UnixStream;
use std::thread::{self, JoinHandle};

use crate::ipc::client_listener::ClientListener;
use crate::logical_device::router::{PhysicalTx, SimulatedTx};
use crate::print_err;

/// Messages received form [`UInput`] or [`Input`].
#[derive(Clone, Debug)]
pub enum LogicalDeviceMessage {}

/// The reason the [`ClientListener`] terminated its loop.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientListenerError {
    /// Socket was unexpectedly disconnected from the client.
    SocketConnection,
    /// The [`ClientListener`] panicked.
    Panicked(String),
    /// The [`ClientListener`] closed unexpectedly.
    Unexpected,
}
impl Display for ClientListenerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientListenerError::SocketConnection => {
                write!(
                    f,
                    "the client listener's socket was disconnected from the client"
                )
            }
            ClientListenerError::Unexpected => {
                write!(f, "unexpected client listener termination")
            }
            ClientListenerError::Panicked(e) => {
                write!(f, "the client listener panicked: {e}")
            }
        }
    }
}
impl Error for ClientListenerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ClientListenerError::SocketConnection => None,
            ClientListenerError::Unexpected => None,
            ClientListenerError::Panicked(_) => None,
        }
    }
}

/// Errors when trying to launch a new client from a socket connection to the process.
#[derive(Debug)]
enum ClientLaunchError {
    /// Failed to clone a socket.
    SocketClone(std::io::Error),
}
impl Display for ClientLaunchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientLaunchError::SocketClone(_) => {
                write!(f, "failed to clone socket for client connection")
            }
        }
    }
}
impl Error for ClientLaunchError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ClientLaunchError::SocketClone(error) => Some(error),
        }
    }
}
/// The id of a connection between the daemon and a process.
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ClientId(u64);
impl ClientId {
    fn next_id(&mut self) {
        self.0 += 1;
    }
}
/// The exit code of a client.
struct ClientExitCode {
    /// The id of the client.
    client_id: ClientId,
    /// The exit code of the client.
    exit_code: Result<(), ClientListenerError>,
}

struct ClientCtx {
    /// The next id to attribute to a client.
    next_client_id: ClientId,

    /// Channel from client to the logical device.
    client_to_dev_tx: SimulatedTx,
    /// The channel the watcher needs to notify the [`Gateway`].
    from_watcher_tx: Sender<ClientExitCode>,
}

/// Holds necesary maps to properly pass messages along.
pub struct Gateway {
    /// The context needed to spawn a new [`ClientListener`].
    client_ctx: ClientCtx,
    /// Map listing the [`ConnectionTx`] for each active session.
    client_map: HashMap<ClientId, ConnectionTx<message::FromDaemon>>,

    /// Message receiver from the [`LogicalDevice`].
    logical_device_rx: Receiver<LogicalDeviceMessage>,
    /// Receives client exit codes from the watcher.
    client_watcher_rx: Receiver<ClientExitCode>,
    /// Receives new connections from [`crate::ipc::connector::listen_socket_server`].
    connector_rx: Receiver<UnixStream>,
}

impl Gateway {
    pub fn new(
        logical_device_rx: Receiver<LogicalDeviceMessage>,
        connector_rx: Receiver<UnixStream>,
        client_to_dev_tx: SimulatedTx,
    ) -> Self {
        let (from_watcher_tx, client_watcher_rx) = crossbeam_channel::unbounded();
        Self {
            client_map: HashMap::new(),
            logical_device_rx,
            client_watcher_rx,
            connector_rx,

            client_ctx: ClientCtx {
                next_client_id: ClientId(0),
                client_to_dev_tx,
                from_watcher_tx,
            },
        }
    }
    /// Launch the routing thread of the app, which mostly handles message passing.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                crossbeam_channel::select_biased! {
                    recv(self.logical_device_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.handle_logical_device_message(m),
                            Err(e) => panic!("unexpected device interface shutdown, closing daemon: {e}"),
                        }
                    },
                    recv(self.client_watcher_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.handle_client_watcher_message(m),
                            Err(e) => panic!("unexpected client listener sender shutdown, closing daemon: {e}"),
                        }
                    },
                    recv(self.connector_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => if let Err(e) = self.launch_new_client(m) {
                                print_err(Box::new(e));
                            },
                            Err(e) => panic!("unexpected connector shutdown, closing daemon: {e}"),
                        }
                    },
                }
            }
        })
    }
    fn handle_logical_device_message(&self, msg: LogicalDeviceMessage) {
        match msg {}
    }
    // TODO: NEXT Turn this into a watcher for client threads.
    fn handle_client_watcher_message(&mut self, msg: ClientExitCode) {
        // let WrappedClientMessage { client_id, message } = msg;
        // match message {
        //     ClientMessage::ClientClosed(client_exit_code) => {
        //         if let Err(e) = client_exit_code {
        //             crate::print_err(Box::new(e));
        //         }
        //         self.client_map.remove(&client_id);
        //         for (_, clients) in self.client_subscription_map.iter_mut() {
        //             clients.retain(|v| *v != client_id);
        //         }
        //     }
        // }
    }
    /// Createa a new client connection given a socket to a client.
    fn launch_new_client(&mut self, stream: UnixStream) -> Result<(), ClientLaunchError> {
        let handle = ClientListener::new(
            ConnectionRx::new(stream.try_clone().map_err(ClientLaunchError::SocketClone)?),
            self.client_ctx.client_to_dev_tx.clone(),
        )
        .launch();

        let client_tx = ConnectionTx::<message::FromDaemon>::new(stream);
        let id = self.client_ctx.next_client_id;
        self.client_map.insert(id, client_tx);

        self.watch_handle(handle, id);
        self.client_ctx.next_client_id.next_id();
        Ok(())
    }
    /// Watch for the death of [`ClientListener`] handles.
    fn watch_handle(
        &mut self,
        handle: JoinHandle<Result<(), ClientListenerError>>,
        client_id: ClientId,
    ) {
        let tx = self.client_ctx.from_watcher_tx.clone();
        thread::spawn(move || match handle.join() {
            Ok(v) => tx.send(ClientExitCode {
                client_id,
                exit_code: v,
            }),
            Err(e) => tx.send(ClientExitCode {
                client_id,
                exit_code: Err(ClientListenerError::Panicked(crate::core::format_panic(&e))),
            }),
        });
    }
}
