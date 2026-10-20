//! Handles the link to the app's API.

use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::ipc::{
    message::FromProcess,
    socket_connection::{ConnectionTx},
};

/// Used as an API for the app's functionalities.
pub struct Handle {
    /// Outgoing connection to the daemon.
    daemon_connection: ConnectionTx<FromProcess>,
}
impl Handle {
    pub(crate) fn new(daemon_connection: ConnectionTx<FromProcess>) -> Self {
        Handle {
            daemon_connection: daemon_connection,
        }
    }
    /// Closes the connection with the daemon.
    pub fn exit(&mut self) {
        if let Err(e) = self.daemon_connection.send(FromProcess::Exit) {
            eprintln!("error: failed to request disconnection");
            eprintln!("  cause: {e}");
        }
    }
    //TODO: Implement api functions
}
/// Wraps a [`Handle`] and add dropping logic.
pub(crate) struct HandleWrapper {
    /// The wrapped [`Handle`].
    handle: Option<Handle>,
    /// The list to return the [`Handle`] to once the wrapper is dropped.
    return_pile: Arc<Mutex<Vec<Handle>>>,
}
impl HandleWrapper {
    pub fn new(handle: Handle, return_pile: Arc<Mutex<Vec<Handle>>>) -> Self {
        HandleWrapper {
            handle: Some(handle),
            return_pile,
        }
    }
}
impl Deref for HandleWrapper {
    type Target = Handle;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("the handle should not be removed until the wrapper is dropped")
    }
}
impl Drop for HandleWrapper {
    fn drop(&mut self) {
        let handle = self
            .handle
            .take()
            .expect("the handle should have been initialized and never taken");
        self.return_pile
            .lock()
            .expect("previous lock should not have panicked")
            .push(handle);
    }
}
