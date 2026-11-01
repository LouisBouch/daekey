//! Defines how the daemon will send input to the compositor.

use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};


/// Messages that can be sent to the uinput interface.
pub enum UInputMessage {}

/// Holds necessary values to interface with uinput and talk back to the [`Router`].
pub struct UInput {
}
impl UInput {
    pub fn new() -> Self {
        Self { }
    }
}
            //let virt_dev = VirtualDevice::builder();
