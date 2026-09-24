//! The hub of communication. Routes messages from uinpu/input to the necessary connected process.

use crossbeam_channel::{Receiver, Sender};
use libdae::input::event::KeyEvent;
use libdae::ipc::{message, socket_connection::ConnectionTx};
use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::thread::JoinHandle;

use crate::device_interface::input::InputMessage;
use crate::device_interface::uinput::UInputMessage;
use crate::ipc::endpoint_listener::SessionId;

/// Messages received form [`UInput`] or [`Input`].
pub enum DeviceInterfaceMessage {}
/// Message Received from a [`Connection`].
pub enum ConnectionMessage {}

/// Holds fields necessary for a [`Connection`].
struct ConnectionContext {
    /// Channel connected to [`crate::device_interface::uinput::UInput`].
    uinput_tx: Sender<UInputMessage>,
    /// Channel connected to [`crate::device_interface::uinput::Input`].
    input_tx: Sender<InputMessage>,
    /// Channel connected to the [`Router`].
    connection_tx: Sender<ConnectionMessage>,
}

/// Holds necesary maps to properly pass messages along.
pub struct Router {
    /// The next id to attribute to a connection/session.
    next_id: u64,
    /// Map listing the [`ConnectionTx`] for each active session.
    endpoint_map: HashMap<SessionId, ConnectionTx<message::FromDaemon>>,
    /// List of connection sessions for each [`KeyEvent`].
    subscription_map: HashMap<KeyEvent, Vec<SessionId>>,

    /// Message receiver from a device interface.
    device_interface_rx: Receiver<DeviceInterfaceMessage>,
    /// Message receiver from an endpoint connection.
    connection_rx: Receiver<ConnectionMessage>,
    /// Receives receiver new connections from the [`Connector`].
    connector_rx: Receiver<UnixStream>,
    /// Will be given to each new [`Connection`].
    connection_ctx: ConnectionContext,
}

impl Router {
    pub fn new(
        device_interface_rx: Receiver<DeviceInterfaceMessage>,
        connection_rx: Receiver<ConnectionMessage>,
        connector_rx: Receiver<UnixStream>,
        uinput_tx: Sender<UInputMessage>,
        input_tx: Sender<InputMessage>,
        connection_tx: Sender<ConnectionMessage>,
    ) -> Self {
        Self {
            next_id: 0,
            endpoint_map: HashMap::new(),
            subscription_map: HashMap::new(),
            device_interface_rx,
            connection_rx,
            connector_rx,
            connection_ctx: ConnectionContext {
                uinput_tx,
                input_tx,
                connection_tx,
            },
        }
    }
    /// Launch the routing thread of the app, which mostly handles message passing.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                crossbeam_channel::select_biased! {
                    recv(self.device_interface_rx) -> c_mes => {
                        let m = c_mes.expect("channel should be open");
                        self.handle_device_interface_message(m);
                    },
                    recv(self.connection_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.handle_connection_message(m),
                            Err(e) => eprintln!("unexpected interface shutdown, closing daemon: {e}"),
                        }
                    },
                    recv(self.connector_rx) -> c_mes => {
                        match c_mes {
                            Ok(m) => self.launch_new_connection(m),
                            Err(e) => eprintln!("unexpected connector shutdown, closing daemon: {e}"),
                        }
                    },
                }
            }
        })
    }
    fn handle_device_interface_message(&self, msg: DeviceInterfaceMessage) {
        match msg {}
    }
    fn handle_connection_message(&mut self, msg: ConnectionMessage) {
        match msg {}
    }
    fn launch_new_connection(&mut self, stream: UnixStream) {}
    /// Shutdown the app.
    fn exit(mut self) {}
}
