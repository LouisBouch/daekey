//! Create bindings through a builder.

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
    sync::Arc,
};

use common_lib::bindings::{ActionId, Binding, BindingPolicy, Bindings};
use common_lib::input::{KeyCode, ModifierMatch, Modifiers};

use crate::{
    action::{Action, BuiltinAction, Runnable},
    daemon_handle::DaemonHandle,
    thread_pool::{ThreadPool, ThreadPoolBuildError},
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
    /// Collisions between [`ModifierMatch::AtLeast`] bindings exist.
    ///
    /// Each key of the [`HashMap`] represents a binding with modifiers that could be pressed during a binding.
    /// The associated value is the set of bound at-least modifiers that conflict with it.
    ModifierCollision(HashMap<(KeyCode, Modifiers), HashSet<Modifiers>>),
}
impl Display for BindingsBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindingsBuildError::DuplicateBinding { key, modifiers } => {
                // TODO: implement better display for key and modifiers?
                write!(
                    f,
                    "failed to build bindings, key: {key:?} with modifiers: {modifiers:?} was bound twice"
                )
            }
            BindingsBuildError::ModifierCollision(hash_map) => todo!(),
        }
    }
}
impl Error for BindingsBuildError {}

/// Bindings built by the builder with prepared actions awaiting initialization.
pub struct PreparedBindings {
    /// Built bindings, ready for the daemon.
    bindings: Bindings,
    /// Defined actions, ready to be built for the client.
    pending_actions: PendingActions,
}
impl PreparedBindings {
    pub(crate) fn deconstruct(self) -> (Bindings, PendingActions) {
        (self.bindings, self.pending_actions)
    }
}
// TODO: make client take a builder when instantiated.
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
    pub fn max_threads_per_binding(&mut self, max_threads: usize) {
        self.max_threads_per_binding = max_threads;
    }
    /// Create a new binding and return a mutable reference to it.
    pub fn bind(&mut self, key: KeyCode, modifiers: ModifierMatch) -> &mut PendingBinding {
        self.pending.push_mut(PendingBinding::new(key, modifiers))
    }
    /// Turn the pending bindings into the key bindings.
    pub fn build(self) -> Result<PreparedBindings, BindingsBuildError> {
        let mut bindings = Bindings::new();
        let mut actions = Vec::new();

        let mut handle_pending_action = |p| match p {
            Some(v) => {
                let id = ActionId::new(actions.len() as u64);
                actions.push(v);
                Some(id)
            }
            None => None,
        };

        for PendingBinding {
            key,
            modifiers,
            policy,
            active_when_paused,
            on_press,
            on_release,
        } in self.pending.into_iter()
        {
            let on_press_id = handle_pending_action(on_press);
            let on_release_id = handle_pending_action(on_release);
            let binding = Binding {
                key,
                modifiers,
                policy,
                active_when_paused,
                on_press: on_press_id,
                on_release: on_release_id,
            };
            if bindings.insert(binding).is_some() {
                return Err(BindingsBuildError::DuplicateBinding { key, modifiers });
            };
        }
        let collisions = bindings.collisions();
        if !collisions.is_empty() {
            return Err(BindingsBuildError::ModifierCollision(collisions));
        }
        Ok(PreparedBindings {
            bindings,
            pending_actions: PendingActions {
                pending_actions: actions,
                default_max_threads: self.max_threads_per_binding,
            },
        })
    }
}
/// Store information about a future binding.
struct PendingBinding {
    key: KeyCode,
    modifiers: ModifierMatch,
    policy: BindingPolicy,
    active_when_paused: bool,
    on_press: Option<PendingAction>,
    on_release: Option<PendingAction>,
}
impl PendingBinding {
    fn new(key: KeyCode, modifiers: ModifierMatch) -> Self {
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
    /// Max amount of threads that can be used to run the closure. `None` if user has not configured
    /// it and will be configured by the builder's default.
    max_threads: Option<usize>,
}
impl PendingRunnable {
    pub fn new(closure: impl Fn(DaemonHandle) + Send + Sync + 'static) -> Self {
        PendingRunnable {
            closure: Arc::new(closure),
            max_threads: None,
        }
    }
    pub fn max_threads(&mut self, max_threads: usize) -> &mut Self {
        self.max_threads = Some(max_threads);
        self
    }
}
/// An action taken by the [`Client`].
enum PendingAction {
    /// Custom closure defined by user.
    Custom(PendingRunnable),
    /// Action that can be executed directly on the thread.
    Builtin(BuiltinAction),
}
/// List of [`PendingAction`] with default amoutn of max threads if not already decided in the
/// pending action.
pub(crate) struct PendingActions {
    /// Default amoutn of threads that will be given to a [`Runnable`] if it isnt already decided in
    /// the [`PendingRunnable`].
    default_max_threads: usize,
    /// List of [`PendingAction`] defined by user.
    pending_actions: Vec<PendingAction>,
}
impl PendingActions {
    /// Turn the list of pending actions into actions the client can use.
    pub fn into_actions(
        self,
        mut ctx_maker: impl FnMut() -> DaemonHandle,
    ) -> Result<Vec<Action>, ThreadPoolBuildError> {
        let mut actions = Vec::new();
        for pending_action in self.pending_actions {
            let action = match pending_action {
                PendingAction::Custom(pending_runnable) => Action::Custom(Runnable {
                    closure: pending_runnable.closure,
                    thread_pool: ThreadPool::try_new(
                        pending_runnable
                            .max_threads
                            .unwrap_or(self.default_max_threads),
                        &mut ctx_maker,
                    )?,
                }),
                PendingAction::Builtin(builtin_action) => Action::Builtin(builtin_action),
            };
            actions.push(action);
        }
        Ok(actions)
    }
}

/// Implementing this trait will allow conversion to a [`PendingAction`].
trait IntoPendingAction {
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
