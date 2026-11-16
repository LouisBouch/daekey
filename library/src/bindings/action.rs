//! Holds different structure pertaining to action that can be excuted.

use std::sync::Arc;

use crate::{daemon_handle::DaemonHandle, thread_pool::ThreadPool};

/// Action that can be executed directly on the thread.
#[derive(Clone, Copy, Debug)]
pub enum BuiltinAction {
    /// Toggles the bindings on or off.
    ToggleBindings,
    /// Disconnects the process from the daemon.
    Exit,
}
/// The closure associated with the action.
pub struct Runnable {
    /// The closure that will run.
    closure: Arc<dyn Fn(DaemonHandle) + Send + Sync>,
    /// Parked threads to be used by closures.
    thread_pool: ThreadPool<DaemonHandle>,
}
/// An action taken by the [`Client`].
pub enum Action {
    /// Run a custom action using a closure.
    Custom(Runnable),
    /// Action that can be executed directly on the thread.
    Builtin(BuiltinAction),
}
