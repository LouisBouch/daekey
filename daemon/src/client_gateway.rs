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
use crate::logical_device::router::{PhysicalTx, SimulatedId, SimulatedTx, SimulatedTxMaker};
use crate::print_err;
use crate::watcher::{ThreadExit, ThreadStatus, Watcher};

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

/// Message received from the logical device.
#[derive(Clone, Debug)]
pub enum FromLogicalDevice {}

/// Holds necesary maps to properly pass messages along.
pub struct Gateway {
    /// Map listing the [`ConnectionTx`] for each active session.
    client_map: HashMap<SimulatedId, ConnectionTx<message::FromDaemon>>,
    /// Watches threads and reports their death.
    watcher: Watcher<SimulatedId, Result<(), ClientListenerError>>,

    /// Creates a new sender that each new [`ClientListener`] will use.
    simulated_tx_maker: SimulatedTxMaker,
    /// Receives message from the logical device.
    logical_device_rx: Receiver<FromLogicalDevice>,

    /// Receives new connections from [`crate::ipc::connector::listen_socket_server`].
    connector_rx: Receiver<UnixStream>,
}

impl Gateway {
    pub fn new(
        logical_device_rx: Receiver<FromLogicalDevice>,
        connector_rx: Receiver<UnixStream>,
        simulated_tx_maker: SimulatedTxMaker,
    ) -> Self {
        Self {
            simulated_tx_maker,
            client_map: HashMap::new(),
            logical_device_rx,
            connector_rx,
            watcher: Watcher::new(),
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
                    recv(self.watcher.rx()) -> c_mes => {
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
    fn handle_logical_device_message(&self, msg: FromLogicalDevice) {
        todo!()
    }
    fn handle_client_watcher_message(
        &mut self,
        msg: ThreadExit<SimulatedId, Result<(), ClientListenerError>>,
    ) {
        let ThreadExit { key, status } = msg;
        match status {
            ThreadStatus::Returned(client_listener_res) => match client_listener_res {
                Ok(_) => (),
                Err(_) => todo!(),
            },
            ThreadStatus::Panicked(_) => todo!(),
        }
        // Clean up device from map.
        self.client_map.remove(&key);
    }
    /// Createa a new client connection given a socket to a client.
    fn launch_new_client(&mut self, stream: UnixStream) -> Result<(), ClientLaunchError> {
        let id = self.simulated_tx_maker.next_id();
        let tx = self.simulated_tx_maker.generate_tx();
        let handle = ClientListener::new(
            ConnectionRx::new(stream.try_clone().map_err(ClientLaunchError::SocketClone)?),
            tx,
        )
        .launch();

        let client_tx = ConnectionTx::<message::FromDaemon>::new(stream);
        self.client_map.insert(id, client_tx);

        self.watcher.watch_thread(id, handle);
        Ok(())
    }
}
