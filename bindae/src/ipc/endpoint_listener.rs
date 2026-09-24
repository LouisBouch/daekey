//! Handles listening of a connected process.

use std::thread::JoinHandle;

use crossbeam_channel::Sender;

use crate::device_interface::{input::InputMessage, uinput::UInputMessage};

/// The id of a connection between the daemon and a process.
pub type SessionId = u64;

/// Info and logic about the receiving side of a connection with a process.
pub struct EndpointListener {
    /// The id of the connection.
    id: SessionId,
    /// Channel to transmit to input.
    input_tx: Sender<InputMessage>,
    /// Channel to transmit to uinput.
    uinput_tx: Sender<UInputMessage>,

}

impl EndpointListener {
    /// Launch the connection, which will start listening to the socket.
    pub fn launch(self) -> JoinHandle<()> {
        std::thread::spawn(move || {})
    }
}
