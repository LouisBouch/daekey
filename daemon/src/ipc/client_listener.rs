//! Handles listening of a connected process.

use std::ops::ControlFlow;
use std::thread::JoinHandle;

use crate::{client_gateway::ClientListenerError, logical_device::router::SimulatedTx};

use library::ipc::message::FromProcess;
use library::ipc::socket_connection::ConnectionRx;

/// Notifies router when dropped.
struct DropGuard {
    /// Channel to transmit to the logical device.
    device_tx: SimulatedTx,
}
impl DropGuard {
    fn new(device_tx: SimulatedTx) -> Self {
        Self { device_tx }
    }
}
impl Drop for DropGuard {
    /// Make sure the router knows the logical device closed so that it can
    /// drop values it stored about it.
    fn drop(&mut self) {
        self.device_tx
            .send(FromProcess::Close)
            .expect("the logical device should not be closed");
    }
}

/// Info and logic about the receiving side of a connection with a process.
pub struct ClientListener {
    /// Receives messages from the client through a [`ConnectionRx`].
    connection_rx: ConnectionRx<FromProcess>,
    /// Channel to transmit to the logical device.
    device_tx: SimulatedTx,
}

impl ClientListener {
    pub fn new(connection_rx: ConnectionRx<FromProcess>, device_tx: SimulatedTx) -> Self {
        Self {
            connection_rx,
            device_tx,
        }
    }
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(mut self) -> JoinHandle<Result<(), ClientListenerError>> {
        std::thread::spawn(move || {
            let mut _guard = DropGuard::new(self.device_tx.clone());
            loop {
                match self.connection_rx.recv() {
                    Ok(m) => {
                        if self.handle_message(m).is_break() {
                            return Ok(());
                        }
                    }
                    Err(e) => {
                        // For a regular socket, DeserializeUnexpectedEnd represents disconnect.
                        if e == postcard::Error::DeserializeUnexpectedEnd {
                            return Err(ClientListenerError::SocketConnection);
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
    pub fn handle_message(&mut self, msg: FromProcess) -> ControlFlow<()> {
        match msg {
            FromProcess::Subscribe => todo!(),
            FromProcess::Close => ControlFlow::Break(()),
            FromProcess::Request(_) => todo!(),
            FromProcess::SetPause(_) => todo!(),
        }
    }
}
