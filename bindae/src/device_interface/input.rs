//! Defines how the daemon will receive inputs from the compositor.

use std::{collections::HashSet, thread::JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use libdae::input::event::KeyEvent;

use crate::{device_interface::uinput::UInputMessage, router::DeviceInterfaceMessage};

/// Messages that can be sent to the input interface.
pub enum InputMessage {}

/// Holds necessary values to interface with input devices and talk back to the [`Router`].
pub struct Input {
    /// Used to send messages to the [`crate::router::Router`].
    router_tx: Sender<DeviceInterfaceMessage>,
    /// Channel used to receive messages.
    rx: Receiver<InputMessage>,
    /// Used to send messages directly to [`crate::device_interface::uinput::UInput`].
    uinput_tx: Sender<UInputMessage>,
    /// List of keys that at least one process subscribed to. Used to decide if it should be sent to
    /// the router or uinput.
    used_keys: HashSet<KeyEvent>,
}
impl Input {
    pub fn new(
        router_tx: Sender<DeviceInterfaceMessage>,
        rx: Receiver<InputMessage>,
        uinput_tx: Sender<UInputMessage>,
    ) -> Self {
        Self {
            router_tx,
            rx,
            uinput_tx,
            used_keys: HashSet::new(),
        }
    }
    /// Update the used keys.
    pub fn set_used_key(&mut self, used_keys: HashSet<KeyEvent>) {
        self.used_keys = used_keys
    }
    /// Launch the interface which will listen to input devices and message requests.
    pub fn launch(self) -> JoinHandle<()> {
        std::thread::spawn(move || {})
    }
}
