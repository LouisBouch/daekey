//! Watches active threads and notifies when one dies.

use std::{
    any::Any,
    thread::{self, JoinHandle},
};

use crossbeam_channel::{Receiver, Sender};

/// The exit status of a thread with its key.
#[derive(Debug)]
pub struct ThreadExit<K, V> {
    /// The key associated with the thread.
    pub key: K,
    /// The exit status.
    pub status: ThreadStatus<V>,
}
/// The Status of a closed thread.
#[derive(Debug)]
pub enum ThreadStatus<V> {
    /// The thread returned manually.
    /// `V` Come from the return value of [`JoinHandle<V>`].
    Returned(V),
    /// The thread panicked.
    /// The `String` is the attempted foramtting of the panic message.
    Panicked(String),
}

/// Converts [`JoinHandle`]s into [`ThreadExit`] message and buffers them into a [`Receiver`].
///
/// Generics:
///
/// * `K` - An identifier type for the threads being watched.
/// * `V` - The return value type of the thread. Same as [`JoinHandle<V>`].
#[derive(Debug)]
pub struct Watcher<K, V> {
    /// Sending side for [`ThreadExit`] messages.
    tx: Sender<ThreadExit<K, V>>,
    /// Receiving side for [`ThreadExit`] messages.
    rx: Receiver<ThreadExit<K, V>>,
}
impl<K: Send + 'static, V: Send + 'static> Watcher<K, V> {
    /// Create a new watcher and return with it
    /// the channel on which notifications will be sent.
    pub fn new() -> Self {
        let (tx, rx) = crossbeam_channel::unbounded();
        Self { tx, rx }
    }
    /// Watch for a thread's termination through its [`JoinHandle`].
    pub fn watch_thread(&self, key: K, handle: JoinHandle<V>) {
        let tx = self.tx.clone();
        let _ = thread::spawn(move || match handle.join() {
            Ok(v) => tx.send(ThreadExit {
                key,
                status: ThreadStatus::Returned(v),
            }),
            Err(e) => tx.send(ThreadExit {
                key,
                status: ThreadStatus::Panicked(format_panic(e.as_ref())),
            }),
        });
    }
    /// Exposes the [`Watcher`]'s receiver.
    /// Holds a list of dead threads in the form of [`ThreadExit`] messages.
    pub fn rx(&self) -> &Receiver<ThreadExit<K, V>> {
        &self.rx
    }
}
impl<K: Send + 'static, V: Send + 'static> Default for Watcher<K, V> {
    fn default() -> Self {
        Self::new()
    }
}
/// Turns a panic payload from a join on a handle into a readable [`String`].
pub fn format_panic(panic: &(dyn Any + Send + 'static)) -> String {
    if let Some(e) = panic.downcast_ref::<String>() {
        e.to_owned()
    } else if let Some(&e) = panic.downcast_ref::<&str>() {
        e.to_owned()
    } else {
        "unknown panic type".to_owned()
    }
}
