//! The hub of communication. Routes messages from uinpu/input to the necessary connected process.
//! Also handles the creation of the client connections in order to keep the routing list up to date.

use crossbeam_channel::{Receiver, SendError, Sender};
use library::input::event::KeyEvent;
use library::ipc::{message, socket_connection::ConnectionRx, socket_connection::ConnectionTx};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;
use std::os::unix::net::UnixStream;
use std::thread::JoinHandle;

use crate::device_interface::input::InputMessage;
use crate::device_interface::uinput::UInputMessage;
use crate::ipc::client_listener::{ClientId, ClientListener};
use crate::print_err;

/// Messages received form [`UInput`] or [`Input`].
#[derive(Clone, Debug)]
pub enum DeviceInterfaceMessage {}

/// The reason the [`ClientListener`] terminated its loop.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum ClientError {
    /// Socket was unexpectedly disconnected from the client.
    SocketConnection,
    /// The [`ClientListener`] panicked.
    Panicked,
    /// The [`ClientListener`] closed unexpectedly.
    Unexpected,
}
impl Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::SocketConnection => {
                write!(
                    f,
                    "the client listener's socket was disconnected from the client"
                )
            }
            ClientError::Unexpected => {
                write!(f, "unexpected client listener termination")
            }
            ClientError::Panicked => {
                write!(f, "the client listener panicked")
            }
        }
    }
}
impl Error for ClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ClientError::SocketConnection => None,
            ClientError::Unexpected => None,
            ClientError::Panicked => None,
        }
    }
}
/// Message Received from a [`ClientListener`].
#[derive(Clone, Debug)]
pub enum ClientMessage {
    ClientClosed(Result<(), ClientError>),
}
/// Wraps a message from a [`ClientListener`] with its id.
#[derive(Clone, Debug)]
pub struct ClientMessageWrapper {
    /// Id of the [`ClientListener`]
    client_id: ClientId,
    /// Message to send.
    message: ClientMessage,
}
/// Wrapper around a client to router sender.
#[derive(Clone, Debug)]
pub struct ClientToRouterTx {
    /// Id of the [`ClientListener`]
    client_id: ClientId,
    /// Sender from [`ClientListener`] to [`Router`].
    sender: Sender<ClientMessageWrapper>,
}
impl ClientToRouterTx {
    pub fn send(&self, message: ClientMessage) -> Result<(), SendError<ClientMessageWrapper>> {
        self.sender.send(ClientMessageWrapper {
            client_id: self.client_id,
            message,
        })
    }
}

/// Holds fields necessary for a [`ClientListener`].
struct ClientContext {
    /// Channel connected to [`crate::device_interface::uinput::UInput`].
    to_uinput_tx: Sender<UInputMessage>,
    /// Channel connected to [`crate::device_interface::uinput::Input`].
    to_input_tx: Sender<InputMessage>,
    /// Channel connected to the [`Router`].
    to_router_tx: Sender<ClientMessageWrapper>,
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

/// Holds necesary maps to properly pass messages along.
pub struct Router {
    /// The next id to attribute to a client.
    next_client_id: ClientId,
    /// Map listing the [`ConnectionTx`] for each active session.
    client_map: HashMap<ClientId, ConnectionTx<message::FromDaemon>>,
    /// List of connection sessions for each [`KeyEvent`].
    client_subscription_map: HashMap<KeyEvent, Vec<ClientId>>,

    /// Message receiver from a device interface.
    device_interface_rx: Receiver<DeviceInterfaceMessage>,
    /// Message receiver from a client connection.
    fr_client_rx: Receiver<ClientMessageWrapper>,
    /// Receives receiver new connections from [`crate::ipc::connector::listen_socket_server`].
    fr_connector_rx: Receiver<UnixStream>,
    /// Will be given to each new [`ClientListener`].
    client_ctx: ClientContext,
}

impl Router {
    pub fn new(
        fr_device_interface_rx: Receiver<DeviceInterfaceMessage>,
        fr_connector_rx: Receiver<UnixStream>,
        to_uinput_tx: Sender<UInputMessage>,
        to_input_tx: Sender<InputMessage>,
    ) -> Self {
        let (fr_client_tx, fr_client_rx) = crossbeam_channel::unbounded();
        Self {
            next_client_id: 0,
            client_map: HashMap::new(),
            client_subscription_map: HashMap::new(),
            device_interface_rx: fr_device_interface_rx,
            fr_client_rx,
            fr_connector_rx,
            client_ctx: ClientContext {
                to_uinput_tx,
                to_input_tx,
                to_router_tx: fr_client_tx,
            },
        }
    }
    /// Launch the routing thread of the app, which mostly handles message passing.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                crossbeam_channel::select_biased! {
                    recv(self.device_interface_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.handle_device_interface_message(m),
                            Err(e) => panic!("unexpected device interface shutdown, closing daemon: {e}"),
                        }
                    },
                    recv(self.fr_client_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.handle_client_message(m),
                            Err(e) => panic!("unexpected client listener sender shutdown, closing daemon: {e}"),
                        }
                    },
                    recv(self.fr_connector_rx) -> c_mes => {
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
    fn handle_device_interface_message(&self, msg: DeviceInterfaceMessage) {
        match msg {}
    }
    fn handle_client_message(&mut self, msg: ClientMessageWrapper) {
        let ClientMessageWrapper { client_id, message } = msg;
        match message {
            ClientMessage::ClientClosed(client_exit_code) => {
                if let Err(e) = client_exit_code {
                    crate::print_err(Box::new(e));
                }
                self.client_map.remove(&client_id);
                for (_, clients) in self.client_subscription_map.iter_mut() {
                    clients.retain(|v| *v != client_id);
                }
            }
        }
    }
    /// Createa a new client connection given a socket to a client.
    fn launch_new_client(
        &mut self,
        stream: UnixStream,
    ) -> Result<JoinHandle<()>, ClientLaunchError> {
        let client_rx = ConnectionRx::<message::FromProcess>::new(
            stream.try_clone().map_err(ClientLaunchError::SocketClone)?,
        );
        let ctx = &self.client_ctx;
        let handle = ClientListener::new(
            client_rx,
            ClientToRouterTx {
                client_id: self.next_client_id,
                sender: ctx.to_router_tx.clone(),
            },
            ctx.to_input_tx.clone(),
            ctx.to_uinput_tx.clone(),
        )
        .launch();

        let client_tx = ConnectionTx::<message::FromDaemon>::new(stream);
        self.client_map.insert(self.next_client_id, client_tx);

        self.next_client_id += 1;
        Ok(handle)
    }
}
