//! Handles listening of a connected process.

use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::{
    device_interface::{input::InputMessage, uinput::UInputMessage},
    router::ConnectionMessage,
};

use libdae::ipc;

/// The id of a connection between the daemon and a process.
pub type SessionId = u64;

/// Info and logic about the receiving side of a connection with a process.
pub struct EndpointListener {
    /// The id of the connection.
    id: SessionId,
    /// The connection with the socket.
    connection: ipc::socket_connection::ConnectionRx<ipc::message::FromProcess>,
    /// Channel to transmit to the router.
    router_tx: Sender<ConnectionMessage>,
    /// Channel to transmit to input.
    input_tx: Sender<InputMessage>,
    /// Channel to transmit to uinput.
    uinput_tx: Sender<UInputMessage>,
}

impl EndpointListener {
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                match self.connection.recv() {
                    Ok(m) => self.handle_message(&m),
                    Err(e) => {
                        eprintln!("Failed to deserialize message, skipping: {e}");
                    }
                }
            }
        })
    }
    /// Handles a message received from the process.
    pub fn handle_message(&mut self, m: &ipc::message::FromProcess) {
        // TODO: Match on messages.
        // match m {}
    }
}
