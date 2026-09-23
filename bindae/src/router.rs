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

/// Holds necesary maps to properly pass messages along.
pub struct Router {
    /// The next id to attribute to a connection/session.
    next_id: u64,
    /// Map listing the [`ConnectionTx`] for each active session.
    endpoint_map: HashMap<SessionId, ConnectionTx<message::FromDaemon>>,
    /// List of connection sessions for each [`KeyEvent`].
    subscription_map: HashMap<KeyEvent, Vec<SessionId>>,
    /// Message from a device interface.
    device_interface_rx: Receiver<DeviceInterfaceMessage>,
    /// Message from an endpoint connection.
    connection_rx: Receiver<ConnectionMessage>,
    /// Receives new connections from the [`Connector`].
    connector_rx: Receiver<UnixStream>,
    /// Channel connected to [`crate::device_interface::uinput::UInput`]. Will be given to each
    /// connected session.
    uinput_tx: Sender<UInputMessage>,
    /// Channel connected to [`crate::device_interface::uinput::Input`]. Will be given to each
    /// connected session.
    input_tx: Sender<InputMessage>,
    /// Channel connected to the [`Router`]. Will be given to each
    /// connected session.
    connection_tx: Sender<ConnectionMessage>,
}

impl Router {
    pub fn new(
        device_interface_rx: Receiver<DeviceInterfaceMessage>,
        connection_rx: Receiver<ConnectionMessage>,
        connector_rx: Receiver<UnixStream>,
        uinput_tx: Sender<UInputMessage>,
        input_tx: Sender<InputMessage>,
        connection_tx: Sender<ConnectionMessage>
    ) -> Self {
        Self {
            next_id: 0,
            endpoint_map: HashMap::new(),
            subscription_map: HashMap::new(),
            device_interface_rx,
            connection_rx,
            connector_rx,
            uinput_tx,
            input_tx,
            connection_tx
        }
    }
    /// Launch the routing thread of the app, which mostly handles message passing.
    pub fn launch(&self) -> JoinHandle<()> {
        // TODO: listen to UInput, Input, the connector AND connections while also being
        // able to send messages to all of them?
        std::thread::spawn(move || {})
    }
}
