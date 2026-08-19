//! The app setup goes through the binder.
use std::{collections::HashMap, sync::Arc, time::Duration};

use evdev::KeyCode;

use crate::{
    api::Api,
    input::{KeyState, Keybind},
    modifiers,
};

/// Holds everything necessary for the app to work.
pub struct Binder {
    /// Maximum number of threads to run closures with.
    max_threads: u16,
    /// The closures for each keybindings.
    bindings: HashMap<Keybind, Arc<dyn Fn(&Api) + Send + Sync>>,
    /// Keybind that toggles the other keybindings.
    toggle_bindings_key: Keybind,
    /// Keybind that exit the program.
    exit_key: Keybind,
    /// Minimum mouse polling interval betwene relative motion events.
    min_mouse_poll_interval: Duration,
    /// Whether the keybinds are paused or not.
    paused: bool,
}
impl Binder {
    pub fn new(max_threads: u16) -> Self {
        let toggle_bindings_key =
            Keybind::new(KeyCode::KEY_PAUSE, KeyState::Pressed, modifiers::NONE);
        let exit_key = Keybind::new(KeyCode::KEY_PAUSE, KeyState::Pressed, modifiers::RIGHT_CTRL);
        Binder {
            max_threads,
            bindings: HashMap::new(),
            toggle_bindings_key,
            exit_key,
            // By default, allow any polling rate.
            min_mouse_poll_interval: Duration::ZERO,
            paused: false,
        }
    }
    /// Create new keybinding.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to bind it to.
    /// * `closure` - The closure that will run when the binding is activated.
    ///
    /// # Return
    ///
    /// [`None`] if the binding did not already exist or the old closure [`Arc<dyn Fn(&Api) + Send + Sync>`] if the binding is overwritting the old binding.
    pub fn create_binding<F>(
        &mut self,
        key: Keybind,
        closure: F,
    ) -> Option<Arc<dyn Fn(&Api) + Send + Sync>>
    where
        F: Fn(&Api) + 'static + Send + Sync,
    {
        self.bindings.insert(key, Arc::new(closure))
    }
    /// Defines a key to turn keybind activation on or off.
    pub fn set_toggle_bindings_key(&mut self, key: Keybind) {
        self.toggle_bindings_key = key;
    }
    /// Defines a key to exit the daemon.
    pub fn set_exit_key(&mut self, key: Keybind) {
        self.exit_key = key;
    }
    /// Set the minimum amount of time that has to pass before sending consecutive
    /// relative mouse movements to the compositor.
    /// This can be used to enhance compatibility with high polling rate mouse with legacy
    /// applications.
    pub fn set_min_mouse_polling_interval(&mut self, interval: Duration) {
        self.min_mouse_poll_interval = interval;
    }

    pub fn max_threads(&self) -> u16 {
        self.max_threads
    }
    pub fn paused(&self) -> bool {
        self.paused
    }
    pub fn exit_key(&self) -> Keybind {
        self.exit_key
    }
    pub fn toggle_bindings_key(&self) -> Keybind {
        self.toggle_bindings_key
    }
    pub fn bindings(&self) -> &HashMap<Keybind, Arc<dyn Fn(&Api) + Send + Sync + 'static>> {
        &self.bindings
    }
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
    pub fn min_mouse_poll_interval(&self) -> Duration {
        self.min_mouse_poll_interval
    }
}
