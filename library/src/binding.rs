//! Defines the content of a binding.

use std::sync::Arc;

use crate::handle::Handle;

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

/// An action taken by the [`Client`].
#[derive(Clone)]
pub enum Action {
    /// Run a closure.
    Closure(Arc<dyn Fn(Handle) + Send + Sync>),
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
struct Edge {
    /// [`Action`] of the [`Edge`].
    action: Action,
    /// Number of threads currently running an [`Action::Closure`].
    active_threads: u32,
    /// Max number of threads to run concurrent [`Action::Closure`]s.
    max_threads: u32,
    /// Whether the key remains bounded after the client paused bindings.
    ///
    /// Defaults to `False`
    active_when_paused: bool,
}
/// The keybind's content.
pub struct Binding {
    /// State associated with the press.
    press: Option<Edge>,
    /// State associated with the release.
    release: Option<Edge>,
    /// Defaults to [`BindingPolicy::Swallow`].
    policy: BindingPolicy,
}
impl Binding {
    /// Creates a new press binding for the press edge.
    /// This binding will ignore the pause state of the client.
    pub fn set_press_active_on_pause(mut self, action: Action, max_threads: u32) -> Self {
        self.press = Some(Edge {
            action,
            max_threads,
            active_when_paused: true,
            active_threads: 0,
        });
        self
    }
    /// Creates a new press binding for the release edge.
    /// This binding will ignore the pause state of the client.
    pub fn set_release_active_on_pause(mut self, action: Action, max_threads: u32) -> Self {
        self.release = Some(Edge {
            action,
            max_threads,
            active_when_paused: true,
            active_threads: 0,
        });
        self
    }
    /// Creates a new press binding for the press edge.
    pub fn set_on_press(mut self, action: Action, max_threads: u32) -> Self {
        self.press = Some(Edge {
            action,
            max_threads,
            active_when_paused: false,
            active_threads: 0,
        });
        self
    }
    /// Creates a new press binding for the release edge.
    pub fn set_on_release(mut self, action: Action, max_threads: u32) -> Self {
        self.release = Some(Edge {
            action,
            max_threads,
            active_when_paused: false,
            active_threads: 0,
        });
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
// /// Info the daemon will require about the binding.
// #[derive(PartialEq, Eq, Clone, Copy)]
// pub struct BindingInfo {
//     /// The edges that have an associated [`Action`].
//     active_edges: ActiveEdges,
//     /// Whether the key should be forwarded or swallowed after triggering the binding.
//     policy: BindingPolicy,
// }
// impl BindingInfo {
//     pub fn active_edges(&self) -> ActiveEdges {
//         self.active_edges
//     }
//     pub fn binding_policy(&self) -> BindingPolicy {
//         self.policy
//     }
// }
