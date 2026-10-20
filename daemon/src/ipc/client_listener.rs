//! Handles listening of a connected process.

use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::{
    device_interface::{input::InputMessage, uinput::UInputMessage},
    router::{ClientError, ClientMessage, ClientToRouterTx},
};

use library::ipc::{message, socket_connection::ConnectionRx};

/// The id of a connection between the daemon and a process.
pub type ClientId = u64;

/// Notifies router when dropped.
struct DropGuard {
    /// Channel to transmit to the router.
    router_tx: ClientToRouterTx,
    /// The exit code of the listener.
    pub exit_status: Result<(), ClientError>,
}
impl DropGuard {
    fn new(router_tx: ClientToRouterTx) -> Self {
        Self {
            router_tx,
            exit_status: Err(ClientError::Unexpected),
        }
    }
}
impl Drop for DropGuard {
    /// Make sure the router knows the [`ClientListener`] closed so that it can
    /// drop values it stored about it.
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.exit_status = Err(ClientError::Panicked);
        }
        // Send will only fail if receiver (router) is closed.
        // Given that only the router cares if a client closes/errors, nobody will be left to care
        // for the sending failure.
        let _ = self
            .router_tx
            .send(ClientMessage::ClientClosed(self.exit_status));
    }
}

/// Info and logic about the receiving side of a connection with a process.
pub struct ClientListener {
    /// Receives messages from the client through a [`ConnectionRx`].
    connection_rx: ConnectionRx<message::FromProcess>,
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
            connection_rx: connection,
            router_tx,
            input_tx,
            uinput_tx,
        }
    }
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            let mut _guard = DropGuard::new(self.router_tx.clone());
            // Number of [`postcard::Error::DeserializeUnexpectedEnd`] errors.
            // If a few in a row occur, assume the other socket has disconnected.
            loop {
                match self.connection_rx.recv() {
                    Ok(m) => match m {
                        message::FromProcess::Exit => {
                            _guard.exit_status = Ok(());
                            break;
                        }
                        m => self.handle_message(m),
                    },
                    Err(e) => {
                        // For a regular socket, DeserializeUnexpectedEnd represents disconnect.
                        if e == postcard::Error::DeserializeUnexpectedEnd {
                            _guard.exit_status = Err(ClientError::SocketConnection);
                            break;
                        }
                        eprintln!(
                            "error: failed to deserialize message from the connected process, skipping"
                        );
                        eprintln!("  cause: {e}");
                    }
                }
            }
        })
    }
    /// Handles a message received from the process.
    pub fn handle_message(&mut self, msg: message::FromProcess) {
        match msg {
            message::FromProcess::Subscribe => todo!(),
            message::FromProcess::Exit => panic!("should be handled by launch match"),
        }
    }
}
