//! Handles listening of a connected process.

use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::{
    device_interface::{input::InputMessage, uinput::UInputMessage},
    router::ConnectionMessage,
};

use libdae::ipc::{message, socket_connection::ConnectionRx};

/// The id of a connection between the daemon and a process.
pub type ClientId = u64;

/// Info and logic about the receiving side of a connection with a process.
pub struct ClientListener {
    /// The id of the connection.
    id: ClientId,
    /// The connection with the socket.
    connection: ConnectionRx<message::FromProcess>,
    /// Channel to transmit to the router.
    router_tx: Sender<ConnectionMessage>,
    /// Channel to transmit to input.
    input_tx: Sender<InputMessage>,
    /// Channel to transmit to uinput.
    uinput_tx: Sender<UInputMessage>,
}

impl ClientListener {
    pub fn new(
        id: ClientId,
        connection: ConnectionRx<message::FromProcess>,
        router_tx: Sender<ConnectionMessage>,
        input_tx: Sender<InputMessage>,
        uinput_tx: Sender<UInputMessage>,
    ) -> Self {
        Self {
            id,
            connection,
            router_tx,
            input_tx,
            uinput_tx,
        }
    }
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            // TODO: Add drop guard to tell router if thread fails. Otherwise, send cleant exit
            // message on one of the handle_message branches.
            loop {
                match self.connection.recv() {
                    Ok(m) => self.handle_message(m),
                    Err(e) => {
                        eprintln!(
                            "error: failed to deserialize message from the connected process, skipping"
                        );
                        eprintln!(" cause: {e}");
                    }
                }
            }
        })
    }
    /// Handles a message received from the process.
    pub fn handle_message(&mut self, msg: message::FromProcess) {
        match msg {}
    }
}
