//! Handles the link to the app's API.

use std::{
    ops::Deref,
    sync::{Arc, Mutex},
};

use crate::{format_error::{ErrorContext, print_err}, ipc::{message::FromProcess, channel::tx::Tx}};

/// Used as an API for the app's functionalities.
pub struct DaemonHandle {
    /// Outgoing connection to the daemon.
    daemon_connection: Tx<FromProcess>,
}
impl DaemonHandle {
    pub(crate) fn new(daemon_connection: Tx<FromProcess>) -> Self {
        DaemonHandle {
            daemon_connection: daemon_connection,
        }
    }
    /// Closes the connection with the daemon.
    pub fn exit(&mut self) {
        if let Err(e) = self.daemon_connection.send(&FromProcess::Close) {
            let ctx = "failed to request disconnection";
            print_err(&ErrorContext::new(ctx.into(), &e));
        }
    }
    //TODO: Implement api functions
}
/// Wraps a [`DaemonHandle`] and add dropping logic.
pub(crate) struct HandleWrapper {
    /// The wrapped [`DaemonHandle`].
    handle: Option<DaemonHandle>,
    /// The list to return the [`DaemonHandle`] to once the wrapper is dropped.
    return_pile: Arc<Mutex<Vec<DaemonHandle>>>,
}
impl HandleWrapper {
    pub fn new(handle: DaemonHandle, return_pile: Arc<Mutex<Vec<DaemonHandle>>>) -> Self {
        HandleWrapper {
            handle: Some(handle),
            return_pile,
        }
    }
}
impl Deref for HandleWrapper {
    type Target = DaemonHandle;

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
