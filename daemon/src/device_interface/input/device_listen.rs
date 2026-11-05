//! Logic to listen to a physical device.

use std::{collections::HashSet, io, path::PathBuf, thread::JoinHandle};

use evdev::{Device, EventSummary, InputEvent, SynchronizationCode};
use nix::libc;

use crate::logical_device::router::{FromPhysicalDevice, PhysicalTx};

use library::ipc::message::Request;

const MAX_SUCCESSIVE_ERRORS: u32 = 20;

/// Notifies router when dropped.
struct DropGuard {
    /// Channel to transmit to the logical device.
    device_tx: PhysicalTx,
}
impl DropGuard {
    fn new(device_tx: PhysicalTx) -> Self {
        Self { device_tx }
    }
}
impl Drop for DropGuard {
    /// Make sure the logical device knows the device closed so that it can
    /// drop values it stored about it.
    fn drop(&mut self) {
        let _ = self.device_tx.send(FromPhysicalDevice::Close);
    }
}

/// Error while running the device listener.
pub enum DeviceError {
    /// The receiver for the logical device is closed.
    LogicalRxClosed,
    /// Too many errors while trying to fetch events from the device.
    FetchEvents { last_err: io::Error },
    /// Failed to get the key states of hte device.
    KeyState { last_err: io::Error },
}

/// A struct containing everything requiring to listen to a physical device.
pub struct DeviceListen {
    /// The [`Device`] to listen to.
    dev: Device,
    /// The path to the [`Device`].
    path: PathBuf,
    /// The channel used to send requests to the logical device.
    tx: PhysicalTx,
    /// Queued up [`Request`]s before a sync.
    requests: Vec<Request>,
    /// Number of successive errors while fetching devic events.
    successive_err: u32,
}
impl DeviceListen {
    pub fn new(dev: Device, path: PathBuf, tx: PhysicalTx) -> Self {
        Self {
            dev,
            path,
            tx,
            requests: Vec::new(),
            successive_err: 0,
        }
    }
    /// Start listening to the [`Device`].
    pub fn listen(mut self) -> JoinHandle<Result<(), DeviceError>> {
        std::thread::spawn(move || {
            let _guard = DropGuard::new(self.tx.clone());
            let retry_delay = std::time::Duration::from_millis(100);
            loop {
                let events = self.dev.fetch_events().map(|v| v.collect::<Vec<_>>());
                match events {
                    Ok(events) => {
                        for event in events {
                            self.handle_event(event)?;
                        }
                        // Resync key presses if at least one error occured.
                        if self.successive_err > 0 {
                            self.resync()?;
                        }
                    }
                    Err(e) => {
                        self.requests.clear();
                        if e.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        if e.raw_os_error() == Some(libc::ENODEV) || !self.path.exists() {
                            return Ok(());
                        }
                        self.successive_err += 1;
                        eprintln!("device error: {e}");
                        if self.successive_err >= MAX_SUCCESSIVE_ERRORS {
                            return Err(DeviceError::FetchEvents { last_err: e });
                        }
                        std::thread::sleep(retry_delay);
                    }
                }
            }
        })
    }
    /// Creates resync request for the logical device.
    ///
    /// Can only fail with [`DeviceError::LogicalRxClosed`].
    fn resync(&mut self) -> Result<(), DeviceError> {
        self.requests.clear();
        match self.dev.get_key_state() {
            Ok(keys) => {
                let pressed_keys = keys.iter().collect::<HashSet<_>>();
                self.tx
                    .send(FromPhysicalDevice::Sync(pressed_keys))
                    .map_err(|_| DeviceError::LogicalRxClosed)?;
                self.successive_err = 0;
                Ok(())
            }
            Err(e) => Err(DeviceError::KeyState { last_err: e }),
        }
    }
    /// Handles an event from the connected device.
    ///
    /// Can only fail with [`DeviceError::LogicalRxClosed`].
    fn handle_event(&mut self, event: InputEvent) -> Result<(), DeviceError> {
        match event.destructure() {
            EventSummary::Synchronization(sync_event, sync_code, _) => {
                match SynchronizationCode(event.code()) {
                    SynchronizationCode::SYN_REPORT => {
                        // Send accumulated requests to the logical device.
                        todo!()
                    }
                    SynchronizationCode::SYN_DROPPED => {
                        self.resync()
                    }
                    _ => Ok(()),
                }
            }
            EventSummary::Key(key_event, key_code, _) => todo!(),
            EventSummary::RelativeAxis(rel_axis_event, rel_axis_code, _) => todo!(),
            EventSummary::AbsoluteAxis(abs_axis_event, abs_axis_code, _) => todo!(),
            _ => Ok(()),
        }
    }
}
