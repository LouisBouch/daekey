//! Defines how the daemon will receive inputs from the compositor.

use std::{collections::HashSet, path::PathBuf, thread::JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use evdev::{BusType, Device, EventType, uinput::VirtualDevice};

use crate::logical_device::router::PhysicalTxMaker;

/// Holds necessary values to interface with input devices and talk back to the [`Router`].
pub struct Input {
    /// Used to create physical senders to the logical device.
    logical_tx: PhysicalTxMaker,
}
impl Input {
    pub fn new(logical_tx: PhysicalTxMaker) -> Self {
        Self { logical_tx }
    }
    /// Launch the interface which will listen to input devices and message requests.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            todo!()
        })
    }
}
//     /// Update the used keys.
//     pub fn set_used_key(&mut self, used_keys: HashSet<Keybind>) {
//         self.used_keys = used_keys
//     }
//     /// Handles a message request for the input interface.
//     pub fn handle_message(&mut self, msg: InputMessage) {
//         match msg {}
//     }
//     /// Obtain the list of devices that will be listened to.
//     fn fetch_devices(&self) {
//         for (path, dev) in evdev::enumerate() {
//             if !Self::is_supported(&dev) {
//                 continue;
//             }
//             let virt_dev = VirtualDevice::builder();
//         }
//     }
//     /// Whether the device is suppoorted by the daemon.
//     fn is_supported(dev: &Device) -> bool {
//         let bus_type = dev.input_id().bus_type();
//         let capabilities = dev.supported_events();
//         if bus_type == BusType::BUS_VIRTUAL
//         // Skip all virtual devices for now, add this to only remove our own.
//         // && dev
//         //     .name()
//         //     .unwrap_or("")
//         //     .contains(library::constants::VIRTUAL_DEVICE_PREFIX)
//         {
//             return false;
//         }
//         if bus_type == BusType::BUS_HOST {
//             return false;
//         }
//         if dev.supported_switches().is_some() {
//             return false;
//         }
//         // Needs to have pressable keys.
//         if !capabilities.contains(EventType::KEY) {
//             return false;
//         }
//         true
//     }
// }

// /// [`evdev::Device`] wrapped with its virtual counterpart.
// struct DaeDevice {
//     /// The path of the device. Used for uniqueness.
//     path: PathBuf,
//     /// The device as seen by [`evdev`].
//     dev: Device,
//     /// The id of the device.
//     id: DeviceId,
// }
