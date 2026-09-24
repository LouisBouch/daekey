//! Defines how the daemon will send input to the compositor.

use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};

use crate::router::DeviceInterfaceMessage;

/// Messages that can be sent to the uinput interface.
pub enum UInputMessage {}

/// Holds necessary values to interface with uinput and talk back to the [`Router`].
pub struct UInput {
    /// Used to send messages to the [`Router`].
    router_tx: Sender<DeviceInterfaceMessage>,
    /// Channel used to receive messages.
    rx: Receiver<UInputMessage>,
}
impl UInput {
    pub fn new(router_tx: Sender<DeviceInterfaceMessage>, rx: Receiver<UInputMessage>) -> Self {
        Self { router_tx, rx }
    }
    /// Launch the interface which will listen to message requests.
    pub fn launch(self) -> JoinHandle<()> {
        std::thread::spawn(move || {})
    }
}
