//! Handles connection between daemon and process using a socket.

use std::{marker::PhantomData, os::unix::net::UnixStream};

use serde::{Serialize, de::DeserializeOwned};

use crate::ipc::message;

/// A connection that sends messages to a connected process.
pub struct ConnectionTx<S: Serialize> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
    /// Generic type has to be held somewhere.
    _t: PhantomData<S>,
}
impl<S: Serialize> ConnectionTx<S> {
    /// Create a new connection sender. This can be used to send data over a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    pub fn new(socket: UnixStream) -> Self {
        Self {
            socket,
            _t: PhantomData,
        }
    }
    /// Send message over the socket.
    pub fn send(&self, msg: S) {
        todo!();
        // TODO: Ensure the message length bit is appended before the message. Also, return a proper Result.
    }
}

/// A connection that receives messages from a connected process.
pub struct ConnectionRx<R: DeserializeOwned> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
    /// Buffer for the receiver.
    buffer: Vec<u8>,
    /// Generic type has to be held somewhere.
    _t: PhantomData<R>,
}
impl<R: DeserializeOwned> ConnectionRx<R> {
    /// Create a new connection receiver. This can be used to receive data from a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    /// * `starting_buffer_size` - The starting size the buffer used for deserializing data into will have.
    pub fn new(socket: UnixStream, starting_buffer_size: usize) -> Self {
        Self {
            socket,
            buffer: Vec::with_capacity(starting_buffer_size),
            _t: PhantomData,
        }
    }
    /// Block while waiting for a new message.
    /// TODO: Add return type.
    pub fn recv(&mut self) {
        todo!();
        // TODO: Make it return the proper message type wrapped in a Result.
    }
}

// TODO: A connection that can both send and receive messages?

// TODO:
// - thread per Connection Rx. Then sends received messages to a logic thread that routes it
// properly.
// - same logic thread receives messages from input/uinput that need to be sent to processes.
