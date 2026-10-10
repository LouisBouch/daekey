//! The stateful client that holds mappings and handles the connection to the daemon.

use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    io,
    sync::{Arc, Mutex},
};

use evdev::KeyEvent;
use rayon::ThreadPoolBuildError;

use crate::{
    connection::{ConnectionError, socket_from_daemon},
    handle::Handle,
};

/// An action taken by the [`Client`].
pub enum Action {
    /// Run a closure using a pool of threads.
    Closure(Arc<dyn Fn(&Handle) + Send + Sync>),
    /// Toggles the bindings on or off.
    ToggleBindings,
    /// Disconnects the process from the daemon.
    Exit,
}
#[derive(Debug)]
/// Possible errors when instantiating a new [`Client`].
pub enum InstantiationError {
    /// Failed to initiate the thread pool.
    ThreadPool(ThreadPoolBuildError),
    /// Failed to connect to the daemon.
    DaemonConnection(ConnectionError),
    /// Failed to duplicate the daemon's socket.
    SocketCloning(io::Error),
}
impl Display for InstantiationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstantiationError::ThreadPool(_) => {
                write!(f, "failed to create thread pool")
            }
            InstantiationError::DaemonConnection(_) => {
                write!(f, "failed to connect to the daemon")
            }
            InstantiationError::SocketCloning(_) => {
                write!(f, "failed to clone the daemon's socket")
            }
        }
    }
}
impl Error for InstantiationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            InstantiationError::ThreadPool(thread_pool_build_error) => {
                Some(thread_pool_build_error)
            }
            InstantiationError::DaemonConnection(connection_error) => Some(connection_error),
            InstantiationError::SocketCloning(error) => Some(error),
        }
    }
}

/// An instance of a connection to the daemon.
pub struct Client {
    /// Parked threads to be used by closures.
    thread_pool: rayon::ThreadPool,
    /// List of handles to be handed to closures.
    handles: Arc<Mutex<Vec<Handle>>>,
    /// Whether the bindings' actions will be executed (The bindings that allow for unpausing are not paused)
    bindings_active: bool,
    /// The [`Action`] associated for each binding's [`KeyEvent`].
    bindings: HashMap<KeyEvent, Action>,
}
impl Client {
    /// Creates a new [`Client`] and connects to the daemon.
    ///
    /// # Aguments
    ///
    /// * `max_threads` - Maximum amount of threads that can run closures at the same time.
    pub fn instantiate(max_threads: usize) -> Result<Self, InstantiationError> {
        let daemon_socket = socket_from_daemon().map_err(InstantiationError::DaemonConnection)?;
        let thread_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(max_threads)
            .build()
            .map_err(InstantiationError::ThreadPool)?;
        let handles = Arc::new(Mutex::new(Vec::new()));
        for _ in 0..max_threads {
            handles
                .lock()
                .expect("previous lock should not have panicked")
                .push(Handle::new(
                    daemon_socket
                        .try_clone()
                        .map_err(InstantiationError::SocketCloning)?,
                ));
        }
        Ok(Self {
            thread_pool,
            handles,
            bindings_active: true,
            bindings: HashMap::new(),
        })
    }
    /// Start the client.
    pub fn run(&mut self) {
        // Add result as return value.
    }
    /// Execute an [`Action`] defined by a [`KeyEvent`] binding.
    fn execute_action(&mut self, action: &Action) {}
}
// TODO: Impl remaining functions.
