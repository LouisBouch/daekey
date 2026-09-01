//! The app setup goes through the binder.
use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::{api::Api, input::Keybind};
/// An action linked to a keybind.
#[derive(Clone)]
pub enum Action {
    Closure(Arc<dyn Fn(&Api) + Send + Sync>),
    /// Pauses and unpauses closures. When paused, pressing a keybind won't trigger the associated closure.
    TogglePauseClosures,
    /// Start recording a macro.
    MacroRecordingStart,
    /// Stop recording a macro.
    MacroRecordingStop,
    /// Exit the app.
    Exit,
}


/// Holds everything necessary for the app to work with bindings.
// TODO: Rethink structure. Binder holds too much currently and will make it harder to reason about
// adding macro handling.
// TODO: Add some start and stop recodring keys.
pub struct Configs {
    /// Maximum number of threads to run closures with.
    max_threads: u16,
    /// The closures for each keybindings.
    bindings: HashMap<Keybind, Action>,
    /// Minimum mouse polling interval betwene relative motion events.
    min_mouse_poll_interval: Duration,
}
impl Configs {
    pub fn new(max_threads: u16) -> Self {
        Configs {
            max_threads,
            bindings: HashMap::new(),
            // By default, allow any polling rate.
            min_mouse_poll_interval: Duration::ZERO,
        }
    }
    /// Create new keybinding with closure.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to bind it to.
    /// * `closure` - The closure that will run when the binding is activated.
    ///
    /// # Return
    ///
    /// [`None`] if the binding did not already exist or the old [`Action`] if the binding is overwritting the old binding.
    pub fn create_closure_binding<F>(&mut self, key: Keybind, closure: F) -> Option<Action>
    where
        F: Fn(&Api) + 'static + Send + Sync,
    {
        self.bindings
            .insert(key, Action::Closure(Arc::new(closure)))
    }
    /// Create new keybinding with an [`Action`].
    ///
    /// # Arguments
    ///
    /// * `key` - The key to bind it to.
    /// * `action` - The action associated with the binding.
    ///
    /// # Return
    ///
    /// [`None`] if the binding did not already exist or the old [`Action`].
    pub fn create_binding(&mut self, key: Keybind, action: Action) -> Option<Action> {
        self.bindings.insert(key, action)
    }
    /// Set the minimum amount of time that has to pass before sending consecutive
    /// relative mouse movements to the compositor.
    /// This can be used to enhance compatibility with high polling rate mouse in legacy
    /// applications.
    ///
    /// NOTE: This cannot got lower than what the mouse offers.
    /// Also, to prevent unwanted latency, set this value as a multiple of your mouse's actual polling interval.
    /// NOTE2: This only affects RELATIVE actions such as X/Y movement, scroll, etc. It does NOT
    /// affect clicks. This is technically innacurate when emulating low polling mice, but
    /// implementing the low polling for clicks as well is just an annoyance (it drops clicks) and does not really
    /// offer additional compatibilty.
    /// NOTE3: This only affects actual devices. If you manually send relative events faster, it will
    /// not throttle.
    pub fn set_rel_mouse_polling_interval(&mut self, interval: Duration) {
        self.min_mouse_poll_interval = interval;
    }
    /// Sets the polling rate of the mouse.
    ///
    /// # Parameters
    ///
    /// * `poll_rate` - The polling rate to emulate, in Hertz.
    ///
    /// NOTE: This cannot go higher that what the mouse offers.
    /// Also, to prevent unwanted latency, set this value as a multiple of your mouse's actual polling interval.
    /// NOTE2: This only affects RELATIVE actions such as X/Y movement, scroll, etc. It does NOT
    /// affect clicks. This is technically innacurate when emulating low polling mice, but
    /// implementing the low polling for clicks as well is just an annoyance (it drops clicks) and does not really
    /// offer additional compatibilty.
    /// NOTE3: This only affects actual devices. If you manually send relative events faster, it will
    /// not throttle.
    pub fn set_rel_mouse_polling_rate(&mut self, poll_rate: u32) {
        self.set_rel_mouse_polling_interval(Duration::from_secs_f64(1.0 / poll_rate as f64));
    }

    pub fn max_threads(&self) -> u16 {
        self.max_threads
    }
    pub fn bindings(&self) -> &HashMap<Keybind, Action> {
        &self.bindings
    }
    pub fn min_mouse_poll_interval(&self) -> Duration {
        self.min_mouse_poll_interval
    }
}
