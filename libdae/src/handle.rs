//! Handles the link to the app's API.

use std::{
    ops::Deref,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

/// Used as an API for the app's functionalities.
pub struct Handle {
    /// Socket connected to the daemon.
    daemon_socket: UnixStream,
}
impl Handle {
    pub fn new(daemon_socket: UnixStream) -> Self {
        Handle { daemon_socket }
    }
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
//TODO: Implement
