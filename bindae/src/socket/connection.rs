//! Defines a connection that is done with a process that makes use of the daemon.

use std::{marker::PhantomData, os::unix::net::UnixStream};

use serde::{Serialize, de::DeserializeOwned};

/// A connection that receives messages from a connected process.
pub struct ConnectionRx<R: DeserializeOwned> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
    /// The struct needs to hold the generic type somewhere.
    _t: PhantomData<R>,
}

/// A connection that sends messages to a connected process.
pub struct ConnectionTx<S: Serialize> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
    /// The struct needs to hold the generic type somewhere.
    _t: PhantomData<S>,
}
// TODO:
// - thread per Connection Rx. Then sends received messages to a logic thread that routes it
// properly.
// - same logic thread receives messages from input/uinput that need to be sent to processes.
