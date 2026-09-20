//! The hub of communication. Routes messages from uinpu/input to the necessary connected process.

use std::collections::HashMap;
use libdae::ipc;

/// Holds necesary maps to properly pass messages along.
pub struct Router {
    /// The next id to attribute to a connection.
    next_id: u64,
    /// Map of connection ids with the associated outgoing connection.
    EndpointMap: HashMap<u64, ipc::socket_connection::ConnectionTx<ipc::message::FromDaemon>>,
    //TODO: Add a receiver to uinput and input.
}

/// Launch the routing thread of the app, which mostly handles message passing.
pub fn launch() {
    // TODO: Find a way to listen to UInput, Input, the connector AND connections while also being
    // able to send messages to all of them.
}


