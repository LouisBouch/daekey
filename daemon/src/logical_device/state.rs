//! Holds the state of the logical device.

use std::collections::{HashMap, HashSet};

use library::keys::{self, KeyCode};

use crate::logical_device::router::DeviceId;

/// The state of a logical device's key.
#[derive(PartialEq, Eq, Clone, Copy, Default)]
struct KeyHolds {
    /// How many simulated devices (coming from client requests) are pressing down the key.
    simulated: u32,
    /// How many physical devices are pressing down the key.
    physical: u32,
}
impl KeyHolds {
    /// Add a key press by a simulated device.
    fn press_simulated(&mut self) {
        self.simulated += 1;
    }
    /// Add a key press by a physical device.
    fn press_physical(&mut self) {
        self.physical += 1;
    }
    /// Release a key press by a simulated device.
    fn release_simulated(&mut self) {
        debug_assert!(
            self.simulated == 0,
            "failure when trying to remove to simulated count, value is already 0"
        );
        self.simulated -= 1;
    }
    /// Release a key press by a physical device.
    fn release_physical(&mut self) {
        debug_assert!(
            self.physical == 0,
            "failure when trying to remove to physical count, value is already 0"
        );
        self.physical -= 1;
    }
    /// Whether any device is holding the key.
    fn is_held(&self) -> bool {
        self.simulated > 0 || self.physical > 0
    }
}

/// The state of the logical device.
pub(super) struct LogicalDeviceState {
    // /// The number of devices holding each key.
    /// key_holds: HashMap<keys::KeyCode, KeyHolds>,
    /// Which keys are being held by which physical device.
    physical_device_keys: HashMap<u64, HashSet<keys::KeyCode>>,
    /// Which keys are being held by which simulated device.
    simulated_device_keys: HashMap<u64, HashSet<keys::KeyCode>>,
}
impl LogicalDeviceState {
    /// Press a key on the logical device.
    ///
    /// Returns true if the key is pressed from a released state.
    pub fn press_key(&mut self, device_id: DeviceId, key_code: KeyCode) -> bool {
        let device_keys = match device_id {
            DeviceId::Simulated(id) => self.simulated_device_keys.entry(id),
            DeviceId::Physical(id) => self.physical_device_keys.entry(id),
        };
        device_keys.or_default().insert(key_code)
    }
    /// Release a key on the logical device.
    ///
    /// Returns true if the key is released from a pressed state.
    pub fn release_key(&mut self, device_id: DeviceId, key_code: KeyCode) -> bool {
        let device_keys = match device_id {
            DeviceId::Simulated(id) => self.simulated_device_keys.get_mut(&id),
            DeviceId::Physical(id) => self.physical_device_keys.get_mut(&id),
        };
        device_keys.is_some_and(|set| set.remove(&key_code))
    }
    /// Remove a device from the logical device and return all keys that were dropped by it.
    pub fn remove_device(&mut self, device_id: DeviceId) -> Vec<KeyCode> {
        let set = match device_id {
            DeviceId::Simulated(id) => self.simulated_device_keys.remove(&id),
            DeviceId::Physical(id) => self.physical_device_keys.remove(&id),
        };
        set.map_or(Vec::new(), |set| set.into_iter().collect())
    }
    /// Whether the key is held by a physical device or not.
    pub fn has_physical_press(&self, key_code: KeyCode) -> bool {
        self.physical_device_keys
            .values()
            .any(|set| set.contains(&key_code))
    }
    /// Whether the key is held by a physical device or not.
    pub fn has_simulated_press(&self, key_code: KeyCode) -> bool {
        self.simulated_device_keys
            .values()
            .any(|set| set.contains(&key_code))
    }
}
