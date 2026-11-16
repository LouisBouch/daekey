//! Create bindings through a builder.

use std::{collections::HashMap, error::Error, fmt::Display, sync::Arc};

use crate::{
    binding::BindingPolicy,
    bindings::{
        action::BuiltinAction,
        keybind::{KeyBindings, ModifierMap},
    },
    daemon_handle::DaemonHandle,
    keys::{self, KeyCode, modifiers::ModifierMatch},
};

const DEFAULT_MAX_THREADS_PER_BINDING: usize = 1;
const DEFAULT_BINDING_POLICY: BindingPolicy = BindingPolicy::Swallow;
const DEFAULT_ACTIVE_ON_PAUSE: bool = false;

/// Error when building the bindings.
#[derive(Debug)]
pub enum BindingsBuildError {
    /// Binding defined twice.
    DuplicateBinding {
        /// Concerned key.
        key: KeyCode,
        /// Associated modifiers.
        modifiers: ModifierMatch,
    },
}
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
// NEXT IMPORTANT: Client should not hold the [`KeyBindings`], the daemon should.
// [`Edges`] Should be changed so that Instead of [`Action`], it holds an ActionId.
// This client will only hold a HashMap of ActionId->Action. This will require a bunch of
// refactoring.
/// Builder for the bindings.
pub struct BindingsBuilder {
    /// Requested bindings.
    pending: Vec<PendingBinding>,
    /// Default value for the maximum amount of threads that can be instantiated per closure-type binding.
    max_threads_per_binding: usize,
}
impl BindingsBuilder {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
            max_threads_per_binding: DEFAULT_MAX_THREADS_PER_BINDING,
        }
    }
    pub fn set_max_threads_per_binding(&mut self, max_threads: usize) {
        self.max_threads_per_binding = max_threads;
    }
    /// Create a new binding and return a mutable reference to it.
    pub fn bind(&mut self, key: KeyCode, modifiers: ModifierMatch) -> &mut PendingBinding {
        self.pending.push_mut(PendingBinding::new(key, modifiers))
    }
    /// Turn the pending bindings into the key bindings.
    pub fn build(self) -> Result<KeyBindings, BindingsBuildError> {
        let mut map = HashMap::new();

        for PendingBinding {
            key,
            modifiers,
            policy,
            active_when_paused,
            on_press,
            on_release,
        } in self.pending
        {
            let modifiers_map = map.entry(key).or_insert(ModifierMap::default());
            if modifiers_map.contains_key(&modifiers) {
                return Err(BindingsBuildError::DuplicateBinding { key, modifiers });
            }
            todo!()
            // Check if colliding modifiers from the atleast map are bound in Exact before
            // throwing an error.
        }
        Ok(KeyBindings(map))
    }
}
/// Store information about a future binding.
pub struct PendingBinding {
    key: keys::KeyCode,
    modifiers: ModifierMatch,
    policy: BindingPolicy,
    active_when_paused: bool,
    on_press: Option<PendingAction>,
    on_release: Option<PendingAction>,
}
impl PendingBinding {
    fn new(key: keys::KeyCode, modifiers: ModifierMatch) -> Self {
        Self {
            key,
            modifiers,
            policy: DEFAULT_BINDING_POLICY,
            active_when_paused: DEFAULT_ACTIVE_ON_PAUSE,
            on_press: None,
            on_release: None,
        }
    }
    /// Change the policy of the binding to `forward`,
    /// which sends the key to the compositor even if bound.
    pub fn forward(&mut self) -> &mut Self {
        self.policy = BindingPolicy::Forward;
        self
    }
    /// Make the binding trigger even when the client is paused.
    pub fn active_when_paused(&mut self) -> &mut Self {
        self.active_when_paused = true;
        self
    }
    pub fn on_press<T: IntoPendingAction>(&mut self, action: T) -> &mut Self {
        self.on_press = Some(action.into_pending_action());
        self
    }
    pub fn on_release<T: IntoPendingAction>(&mut self, action: T) -> &mut Self {
        self.on_release = Some(action.into_pending_action());
        self
    }
}

/// The closure associated with the action.
pub struct PendingRunnable {
    /// The closure that will run.
    closure: Arc<dyn Fn(DaemonHandle) + Send + Sync>,
    /// Max amount of threads that can be used to run the closure.
    max_threads: Option<usize>,
}
impl PendingRunnable {
    pub fn new(closure: impl Fn(DaemonHandle) + Send + Sync + 'static) -> Self {
        PendingRunnable {
            closure: Arc::new(closure),
            max_threads: None,
        }
    }
}
/// An action taken by the [`Client`].
pub enum PendingAction {
    /// Run a custom action using a closure.
    Custom(PendingRunnable),
    /// Action that can be executed directly on the thread.
    Builtin(BuiltinAction),
}

/// Implementing this trait will allow conversion to a [`PendingAction`].
pub trait IntoPendingAction {
    /// Converts [`self`] to a [`PendingAction`].
    fn into_pending_action(self) -> PendingAction;
}
impl IntoPendingAction for BuiltinAction {
    fn into_pending_action(self) -> PendingAction {
        PendingAction::Builtin(self)
    }
}
impl IntoPendingAction for PendingRunnable {
    fn into_pending_action(self) -> PendingAction {
        PendingAction::Custom(self)
    }
}
impl<F> IntoPendingAction for F
where
    F: Fn(DaemonHandle) + Send + Sync + 'static,
{
    fn into_pending_action(self) -> PendingAction {
        PendingRunnable::new(self).into_pending_action()
    }
}
// TODO: Prevent user form calling b.bind(key1, mod1).on_press() and then  b.bind(key1, mod1).on_release()
