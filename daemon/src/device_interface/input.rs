//! Defines how the daemon will receive inputs from the compositor.

use std::{collections::HashSet, path::PathBuf, thread::JoinHandle};

use crate::{
    device_interface::input::device_listen::{DeviceError, DeviceListen}, logical_device::router::PhysicalTxMaker, watcher::{ThreadExit, ThreadStatus, Watcher}
};

use evdev::{BusType, Device, EventType};
mod device_listen;

/// Holds necessary values to interface with input devices and talk back to the [`Router`].
pub struct Input {
    /// Used to create physical senders to the logical device.
    physical_tx_maker: PhysicalTxMaker,
    /// Path to active devices. Starts with `/dev/input`.
    devices: HashSet<PathBuf>,
    /// Watches threads and reports their death.
    watcher: Watcher<PathBuf, Result<(), DeviceError>>,
}
impl Input {
    pub fn new(logical_tx: PhysicalTxMaker) -> Self {
        Self {
            physical_tx_maker: logical_tx,
            devices: HashSet::new(),
            watcher: Watcher::new(),
        }
    }
    /// Launch the interface which will listen to input devices and update requests
    /// from the compositor interface.
    pub fn launch(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            crossbeam_channel::select_biased! {
                recv(self.watcher.rx()) -> c_mes => {
                    match c_mes {
                        Ok(m) => {
                            let ThreadExit { key, status } = m;
                            match status {
                                ThreadStatus::Returned(v) => match v {
                                    Ok(_) => todo!(),
                                    Err(_) => todo!(),
                                },
                                ThreadStatus::Panicked(e) => {
                                    eprintln!("thread watching device `{key:?}` panicked: {e}")
                                },
                            }
                            self.devices.remove(&key);
                            self.update_listened();
                        },
                        Err(e) => panic!("unexpected watcher shutdown, closing input: {e}"),
                    }
                },
            }
        })
    }
    /// Start listening to devices that are not being listened to already.
    fn update_listened(&mut self) {
        for (path, dev) in evdev::enumerate() {
            // Skip device if not supported or if it is already being listened to.
            if !(is_supported(&dev) && self.devices.insert(path.clone())) {
                continue;
            }
            let handle =
                DeviceListen::new(dev, path.clone(), self.physical_tx_maker.generate_tx()).listen();
            self.watcher.watch_thread(path, handle);
        }
    }
}
/// Whether the device is suppoorted by the daemon.
fn is_supported(dev: &Device) -> bool {
    let bus_type = dev.input_id().bus_type();
    let capabilities = dev.supported_events();
    if bus_type == BusType::BUS_VIRTUAL
    // Skip all virtual devices for now, add this to only remove our own.
    // && dev
    //     .name()
    //     .unwrap_or("")
    //     .contains(library::constants::VIRTUAL_DEVICE_PREFIX)
    {
        return false;
    }
    if bus_type == BusType::BUS_HOST {
        return false;
    }
    if dev.supported_switches().is_some() {
        return false;
    }
    // Needs to have pressable keys.
    if !capabilities.contains(EventType::KEY) {
        return false;
    }
    true
}
