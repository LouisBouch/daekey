//! Defines the content of a binding.

use std::sync::{Arc, Mutex};

use crate::daemon_handle::DaemonHandle;

/// What to do with the event if it is bound.
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum BindingPolicy {
    /// Trigger the binding and then forward the key to the compositor.
    Forward,
    /// Trigger the binding but do no forward to compositor.
    Swallow,
}
impl Default for BindingPolicy {
    fn default() -> Self {
        Self::Swallow
    }
}
// TODO: Make custom threadpool which contains the handle. this will make it self contained.
// Maybe use generic type instead of baked in DaemonHandle. This then becomes some kind of executor.
// Give same generic to threadpool.
/// The closure associated with the action.
pub struct ClosureAction {
    /// The closure that will run.a TODO: NEXT Figure out the shape of it. (list of handles? one handle
    /// and share it? somethig else?)
    closure: Arc<dyn Fn(DaemonHandle) + Send + Sync>,
    /// Parked threads to be used by closures.
    thread_pool: rayon::ThreadPool,
    /// List of handles to be handed to closures.
    handles: Arc<Mutex<Vec<DaemonHandle>>>,
}
/// An action taken by the [`Client`].
pub enum Action {
    /// Run a closure.
    Closure(ClosureAction),
    /// Toggles the bindings on or off.
    ToggleBindings,
    /// Disconnects the process from the daemon.
    Exit,
}

/// Which edge has as [`Action`].
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ActiveEdges {
    /// Only a press [`Action`].
    Press,
    /// Only a release [`Action`].
    Release,
    /// Action on press and release.
    Both,
    /// Action on neither.
    None,
}
/// The keybind's content per edge.
pub struct Edges {
    /// State associated with the press.
    press: Option<Action>,
    /// State associated with the release.
    release: Option<Action>,
    /// Defaults to [`BindingPolicy::Swallow`].
    policy: BindingPolicy,
    /// Whether the key remains bound after the client paused bindings.
    ///
    /// Defaults to `False`
    active_when_paused: bool,
}
impl Edges {
    // /// Creates a new press binding for the press edge.
    // /// This binding will ignore the pause state of the client.
    // pub fn set_press_active_on_pause(mut self, action: Action, max_threads: u32) -> Self {
    //     self.press = Some(Binding {
    //         action,
    //         max_threads,
    //         active_when_paused: true,
    //         active_threads: 0,
    //     });
    //     self
    // }
    // /// Creates a new press binding for the release edge.
    // /// This binding will ignore the pause state of the client.
    // pub fn set_release_active_on_pause(mut self, action: Action, max_threads: u32) -> Self {
    //     self.release = Some(Binding {
    //         action,
    //         max_threads,
    //         active_when_paused: true,
    //         active_threads: 0,
    //     });
    //     self
    // }
    // Creates a new press binding for the press edge.
    pub fn set_on_press(mut self, action: Action) -> Self {
        self.press = Some(action);
        self
    }
    /// Creates a new press binding for the release edge.
    pub fn set_on_release(mut self, action: Action) -> Self {
        self.release = Some(action);
        self
    }
    /// Set the new policy.
    pub fn set_policy(mut self, policy: BindingPolicy) -> Self {
        self.policy = policy;
        self
    }
    /// Which edges currently have a binding?
    pub fn active_edges(&self) -> ActiveEdges {
        if self.press.is_some() {
            ActiveEdges::Press
        } else if self.press.is_some() && self.release.is_some() {
            ActiveEdges::Both
        } else if self.release.is_some() {
            ActiveEdges::Release
        } else {
            ActiveEdges::None
        }
    }
}
