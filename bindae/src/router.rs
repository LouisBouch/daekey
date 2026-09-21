//! The hub of communication. Routes messages from uinpu/input to the necessary connected process.

use crossbeam_channel::Receiver;
use libdae::input::event::KeyEvent;
use libdae::ipc::{message, socket_connection::ConnectionTx};
use std::collections::HashMap;

use crate::ipc::endpoint_listener::SessionId;

/// Holds necesary maps to properly pass messages along.
pub struct Router {
    /// The next id to attribute to a connection/session.
    next_id: u64,
    /// Map listing the [`ConnectionTx`] for each active session.
    endpoint_map: HashMap<SessionId, ConnectionTx<message::FromDaemon>>,
    /// List of connection sessions for each [`KeyEvent`].
    subscription_map: HashMap<KeyEvent, Vec<SessionId>>,
    /// Channel receiving from the endpoint listener.
    endpoint_rx: Receiver<RouterMessage>,
    /// Channel receiving messages to route to the correct connnected process.
    route_rx: Receiver<message::FromDaemon>,
}

/// Launch the routing thread of the app, which mostly handles message passing.
pub fn launch() {
    // TODO: listen to UInput, Input, the connector AND connections while also being
    // able to send messages to all of them?
}

/// Messages that can be sent to the router.
pub enum RouterMessage {}
