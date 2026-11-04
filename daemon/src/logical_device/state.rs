//! Holds the state of the logical device.

use std::collections::{HashMap, HashSet};

use library::keys::{self, KeyCode};

use crate::logical_device::router::{PhysicalId, SimulatedId};

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
/// A list of keys that were pressed are a list of keys that were released.
pub struct KeyChanges {
    /// List of pressed keys.
    pub pressed: Vec<KeyCode>,
    /// List of released keys.
    pub released: Vec<KeyCode>,
}

enum Id {
    Simulated(SimulatedId),
    Physical(PhysicalId),
}
/// The state of the logical device.
pub(super) struct LogicalDeviceState {
    // /// The number of devices holding each key.
    /// key_holds: HashMap<keys::KeyCode, KeyHolds>,
    /// Which keys are being held by which physical device.
    physical_device_keys: HashMap<PhysicalId, HashSet<keys::KeyCode>>,
    /// Which keys are being held by which simulated device.
    simulated_device_keys: HashMap<SimulatedId, HashSet<keys::KeyCode>>,
}
impl LogicalDeviceState {
    /// Press a key on a simulated device.
    ///
    /// Returns `true` if the device associated with the [`SimulatedId`] was not
    /// already holding the key and `false` if the device was holding the key.
    pub fn press_simulated_key(&mut self, id: SimulatedId, key_code: KeyCode) -> bool {
        let device_keys = self.simulated_device_keys.entry(id);
        device_keys.or_default().insert(key_code)
    }
    /// Press a key on a physical device.
    ///
    /// Returns `true` if the device associated with the [`PhysicalId`] was not
    /// already holding the key and `false` if the device was pressing the key.
    pub fn press_physical_key(&mut self, id: PhysicalId, key_code: KeyCode) -> bool {
        let device_keys = self.physical_device_keys.entry(id);
        device_keys.or_default().insert(key_code)
    }
    /// Release a key on the a simulated device.
    ///
    /// Returns `true` if the device associated with the [`SimulatedId`] was
    /// holding the key and `false` if the device was not pressing the key.
    pub fn release_simulated_key(&mut self, id: SimulatedId, key_code: KeyCode) -> bool {
        let device_keys = self.simulated_device_keys.get_mut(&id);
        device_keys.is_some_and(|set| set.remove(&key_code))
    }
    /// Release a key on the a physical device.
    ///
    /// Returns `true` if the device associated with the [`PhysicalId`] was
    /// holding the key and `false` if the device was not pressing the key.
    pub fn release_physical_key(&mut self, id: PhysicalId, key_code: KeyCode) -> bool {
        let device_keys = self.physical_device_keys.get_mut(&id);
        device_keys.is_some_and(|set| set.remove(&key_code))
    }
    /// Remove a simulated device from the logical device and return all keys that were dropped by it.
    pub fn remove_simulated_device(&mut self, id: SimulatedId) -> Vec<KeyCode> {
        let set = self.simulated_device_keys.remove(&id);
        set.map_or(Vec::new(), |set| set.into_iter().collect())
    }
    /// Remove a physical device from the logical device and return all keys that were dropped by it.
    pub fn remove_physical_device(&mut self, id: PhysicalId) -> Vec<KeyCode> {
        let set = self.physical_device_keys.remove(&id);
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
    /// Set the list of pressed keys for a device.
    ///
    /// Returns the list of keys that are newly pressed and the list of keys that used to be pressed
    /// but are not anymore.
    pub fn set_device_presses(&mut self, id: Id, keys: Vec<KeyCode>) -> KeyChanges {
        let mut pressed = Vec::new();
        let mut released = Vec::new();

        let mut set = match id {
            Id::Simulated(simulated_id) => self
                .simulated_device_keys
                .remove(&simulated_id)
                .unwrap_or_default(),
            Id::Physical(physical_id) => self
                .physical_device_keys
                .remove(&physical_id)
                .unwrap_or_default(),
        };
        let mut new_set = HashSet::new();
        for key in keys {
            new_set.insert(key);
            if set.remove(&key) {
                continue;
            }
            pressed.push(key);
        }
        for key in set.iter() {
            released.push(*key);
        }
        match id {
            Id::Simulated(simulated_id) => self.simulated_device_keys.insert(simulated_id, new_set),
            Id::Physical(physical_id) => self.physical_device_keys.insert(physical_id, new_set),
        };

        KeyChanges { pressed, released }
    }
    /// Set the list of pressed keys for a physical device.
    ///
    /// Returns the list of keys that are newly pressed and the list of keys that used to be pressed
    /// but are not anymore.
    pub fn set_physical_presses(&mut self, id: PhysicalId, keys: Vec<KeyCode>) -> KeyChanges {
        self.set_device_presses(Id::Physical(id), keys)
    }
}
