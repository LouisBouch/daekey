//! Defines how the daemon will receive inputs from the compositor.

use std::{collections::HashSet, os::fd::AsFd, path::PathBuf, thread::JoinHandle};

use crossbeam_channel::{Receiver, Sender};
use evdev::{
    BusType, Device, EventSummary, EventType, InputEvent, SynchronizationCode,
    uinput::VirtualDevice,
};
use nix::{
    errno::Errno,
    libc,
    poll::{PollFlags, PollTimeout, poll},
};

use crate::{
    logical_device::router::{FromPhysicalDevice, PhysicalTx, PhysicalTxMaker},
    watcher::{ThreadExit, ThreadStatus, Watcher},
};

const MAX_SUCCESSIVE_ERRORS: u32 = 20;

/// Holds necessary values to interface with input devices and talk back to the [`Router`].
pub struct Input {
    /// Used to create physical senders to the logical device.
    physical_tx_maker: PhysicalTxMaker,
    /// Path to active devices. Starts with `/dev/input`.
    devices: HashSet<PathBuf>,
    /// Watches threads and reports their death.
    watcher: Watcher<PathBuf, ()>,
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
                                ThreadStatus::Returned(_) => (),
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
                device_listen(dev, path.clone(), self.physical_tx_maker.generate_tx());
            self.watcher.watch_thread(path, handle);
        }
    }
}


/// Creates resync request for the logical device.
fn resync(device: &Device) {
    todo!()
}
/// Start listening to a specific device defined by its path.
// TODO: Add return type to join handle?
fn device_listen(mut dev: Device, path: PathBuf, tx: PhysicalTx) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut successive_err = 0;
        let retry_delay = std::time::Duration::from_millis(100);
        let mut requests: Vec<FromPhysicalDevice> = Vec::new();
        loop {
            match dev.fetch_events() {
                Ok(events) => {
                    // TODO: resync device if there was an error (err>0).
                    // find a way to please borrow checker.
                    successive_err = 0;
                    for event in events {
                        handle_event(event, &mut requests);
                    }
                }
                Err(e) => {
                    requests.clear();
                    if e.kind() == std::io::ErrorKind::Interrupted {
                        continue;
                    }
                    if e.raw_os_error() == Some(libc::ENODEV) || !path.exists() {
                        return;
                    }
                    successive_err += 1;
                    if successive_err >= MAX_SUCCESSIVE_ERRORS {
                        return;
                    }
                    std::thread::sleep(retry_delay);
                }
            }
        }
    })
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
fn handle_event(event: InputEvent, requests: &mut Vec<FromPhysicalDevice>) {
    match event.destructure() {
        EventSummary::Synchronization(sync_event, sync_code, _) => {
            match SynchronizationCode(event.code()) {
                SynchronizationCode::SYN_REPORT => {
                    // Send accumulated requests to the logical device.
                    todo!()
                }
                SynchronizationCode::SYN_DROPPED => {
                    // Drop current accumulated requests and then resync the
                    // entire device.
                    // Also call this clearing if error occurs.
                    todo!()
                }
                _ => return,
            }
        }
        EventSummary::Key(key_event, key_code, _) => todo!(),
        EventSummary::RelativeAxis(rel_axis_event, rel_axis_code, _) => todo!(),
        EventSummary::AbsoluteAxis(abs_axis_event, abs_axis_code, _) => todo!(),
        _ => return,
    }
}
