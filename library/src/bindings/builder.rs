//! Create bindings through a builder.

use std::{collections::HashMap, error::Error, fmt::Display, sync::{Arc, Mutex}};

use crate::{
    binding::{Action, BindingPolicy}, daemon_handle::DaemonHandle, keys::{
        self, KeyCode,
        modifiers::{ModifierMatch, Modifiers},
    }, thread_pool
};

/// Error when building the bindings.
#[derive(Debug)]
pub enum BindingsBuildError {}
impl Display for BindingsBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}
impl Error for BindingsBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        todo!()
    }
}

// TODO: make client take a builder when instantiated.
/// Builder for the bindings.
pub struct BindingsBuilder {
    /// Requested bindings.
    pending: Vec<PendingBinding>,
    /// Default value for the maximum amount of threads that can be instantiated per closure-type binding.
    default_max_threads_per_binding: u32,
}
/// Store information about a future [`Binding`].
pub struct PendingBinding {
    key: keys::KeyCode,
    modifiers: ModifierMatch,
    policy: BindingPolicy,
    active_when_paused: bool,
    edges: Edges,
    max_threads_per_binding: u32,
}
impl PendingBinding {
    /// Change the policy of the binding to `forward`,
    /// which sends the key to the compositor even if bound.
    pub fn forward(&mut self) {
        self.policy = BindingPolicy::Forward;
    }
    /// Make the binding trigger even when the client is paused.
    pub fn active_when_paused(&mut self) {
        self.active_when_paused = true;
    }
    pub fn on_press(&mut self, action: Action) {
        self.edges.press = Some(action);
    }
    pub fn on_release(&mut self, action: Action) {
        self.edges.release = Some(action);
    }
}

// TODO: Move the definitions somewhere appropriate.
struct KeyBindings(HashMap<KeyCode, ModifierMap>);
struct ModifierMap {
    exact: HashMap<Modifiers, Edges>,
    at_least: Vec<(Modifiers, Edges)>,
}
struct Edges {
    press: Option<Action>,
    release: Option<Action>,
    policy: BindingPolicy,
    active_when_paused: bool,
}
struct ThreadPool {}

/// The closure associated with the action.
pub struct ClosureAction {
    /// The closure that will run. TODO: Use custom pool instead that also holds handles.
    closure: Arc<dyn Fn(DaemonHandle) + Send + Sync>,
    /// Parked threads to be used by closures.
    thread_pool: ThreadPool,
    /// List of handles to be handed to closures.
    handles: Arc<Mutex<Vec<DaemonHandle>>>,
}
