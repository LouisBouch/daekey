//! Handles listening of a connected process.

use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::{
    device_interface::{input::InputMessage, uinput::UInputMessage},
    router::{ClientExitCode, ClientMessage, ClientToRouterTx},
};

use libdae::ipc::{message, socket_connection::ConnectionRx};

/// The id of a connection between the daemon and a process.
pub type ClientId = u64;

/// Notifies router when dropped.
struct DropGuard {
    /// Channel to transmit to the router.
    router_tx: ClientToRouterTx,
    /// The exit code of the listener.
    pub exit_code: ClientExitCode,
}
impl DropGuard {
    fn new(router_tx: ClientToRouterTx) -> Self {
        Self {
            router_tx,
            exit_code: ClientExitCode::Unexpected,
        }
    }
}
impl Drop for DropGuard {
    /// Make sure the router knows the [`ClientListener`] closed so that it can
    /// drop values it stored about it.
    fn drop(&mut self) {
        // Only errors out if receiver (router) is closed.
        // Given that only the router cares if a client is dropped, nobody is
        // left to care in such a case. :(
        let _ = self
            .router_tx
            .send(ClientMessage::ClientClosed(self.exit_code));
    }
}

/// Info and logic about the receiving side of a connection with a process.
pub struct ClientListener {
    /// The connection with the socket.
    connection: ConnectionRx<message::FromProcess>,
    /// Channel to transmit to the router.
    router_tx: ClientToRouterTx,
    /// Channel to transmit to input.
    input_tx: Sender<InputMessage>,
    /// Channel to transmit to uinput.
    uinput_tx: Sender<UInputMessage>,
}

impl ClientListener {
    pub fn new(
        connection: ConnectionRx<message::FromProcess>,
        router_tx: ClientToRouterTx,
        input_tx: Sender<InputMessage>,
        uinput_tx: Sender<UInputMessage>,
    ) -> Self {
        Self {
            connection,
            router_tx,
            input_tx,
            uinput_tx,
        }
    }
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let _guard = DropGuard::new(self.router_tx.clone());
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
