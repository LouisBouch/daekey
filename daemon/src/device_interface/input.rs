//! Defines how the daemon will receive inputs from the compositor.

use std::{collections::HashSet, thread::JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use evdev::{BusType, EventType};
use library::input::event::KeyEvent;

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
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                match self.rx.recv() {
                    Ok(m) => self.handle_message(m),
                    Err(e) => {
                        eprintln!(
                            "error: failed to deserialize message for the input interface, skipping"
                        );
                        eprintln!("  cause: {e}");
                    }
                }
            }
        })
    }
    /// Handles a message request for the input interface.
    pub fn handle_message(&mut self, msg: InputMessage) {
        match msg {}
    }
    /// Obtain the list of devices that will be listened to.
    fn find_devices(&self) {
        for (path, dev) in evdev::enumerate() {
            let capabilities = dev.supported_events();
            let bus_type = dev.input_id().bus_type();
            // Skip our own virtual devices.
            if bus_type == BusType::BUS_VIRTUAL
                // Skip all virtual devices for now, add this to only remove our own.
                // && dev
                //     .name()
                //     .unwrap_or("")
                //     .contains(library::constants::VIRTUAL_DEVICE_PREFIX)
            {
                continue;
            }
            if bus_type == BusType::BUS_HOST {
                continue;
            }
            if dev.supported_switches().is_some() {
                continue;
            }
            // Needs to have pressable keys.
            if !capabilities.contains(EventType::KEY) {
                continue;
            }
        }
    }
}
/// Device defined by the defined.
struct DaeDevice {}
// TODO: NEXT start listening for keys.
// To do so, create [`DaeDevice`] which wrap a device, and grab them.
// Clone each device (and give it to DaeDevice), so that when a key is not bound, it can be sent to the cloned virtual device
// right away. Those devices will live in Input. Add dropguard for grabbing.
//
// As for UInput, create 3 virt dvices (keyboard, abs mouse, rel mouse) but expose to the user
// "SendKey", "SendButton?", "MoveRelative", and "MoveAbsolute".
