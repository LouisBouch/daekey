//! Sends messages by serializing and using a wrapped socket.

use std::{
    error::Error,
    fmt::Display,
    io::Write,
    marker::PhantomData,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use serde::Serialize;

use super::MAX_MSG_LENGTH;
use super::PREFIX_LEN;

const DEFAULT_TX_BUFFER_SIZE: usize = 1024;

/// Represents an error while trying to send a message.
#[derive(Debug)]
pub enum SendError {
    /// Failed to write to socket.
    Write(std::io::Error),
    /// The sender is bricked and cannot send other messages.
    Bricked,
}
impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Write(_) => {
                write!(f, "failed to send message over socket")
            }
            SendError::Bricked => {
                write!(
                    f,
                    "cannot send message, the sender has already encountered a fatal error"
                )
            }
        }
    }
}
impl Error for SendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendError::Write(error) => Some(error),
            SendError::Bricked => None,
        }
    }
}

/// Failed to serialize a value before sending.
#[derive(Debug)]
pub enum SerializeError {
    /// [`postcard`] failed to serialize the value.
    Postcard(postcard::Error),
    /// Serialized output exceeded the limit.
    ///
    /// * `usize` - Length of the message that was supposed to send, in bytes.
    OutputTooLarge(usize),
}
impl Display for SerializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SerializeError::Postcard(_) => {
                write!(f, "failed to serialize message with postcard")
            }
            SerializeError::OutputTooLarge(len) => {
                write!(
                    f,
                    "serialized output size exceeded limit. has size {len} byte(s) with max size {MAX_MSG_LENGTH} bytes"
                )
            }
        }
    }
}
impl Error for SerializeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SerializeError::Postcard(error) => Some(error),
            SerializeError::OutputTooLarge(_) => None,
        }
    }
}

/// A connection that sends messages to a connected process.
#[derive(Clone)]
pub struct Tx<S: Serialize> {
    /// Socket connecting the processes.
    socket: Arc<Mutex<UnixStream>>,
    /// Whether the sender has encountered a fatal error.
    bricked: bool,
    /// Generic type has to be held somewhere.
    _t: PhantomData<S>,
    /// Used to serialize data into.
    buffer: Option<Vec<u8>>,
}
impl<S: Serialize> Tx<S> {
    /// Create a new connection sender. This can be used to send data over a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    pub fn new(socket: UnixStream) -> Self {
        Self::with_starting_buffer_capacity(socket, DEFAULT_TX_BUFFER_SIZE)
    }
    /// Create a new connection sender. This can be used to send data over a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    /// * `starting_buffer_capacity` - The starting capacity of the temporary buffer which holds
    /// serialized data.
    pub fn with_starting_buffer_capacity(
        socket: UnixStream,
        starting_buffer_capacity: usize,
    ) -> Self {
        Self {
            socket: Arc::new(Mutex::new(socket)),
            bricked: false,
            _t: PhantomData,
            buffer: Some(Vec::with_capacity(starting_buffer_capacity)),
        }
    }
    /// Send message over the socket.
    pub fn send(&mut self, msg: &S) -> Result<Result<(), SerializeError>, SendError> {
        if self.bricked {
            return Err(SendError::Bricked);
        }
        self.send_inner(msg).inspect_err(|_| {
            let _ = self
                .socket
                .lock()
                .expect("mutex should lock")
                .shutdown(std::net::Shutdown::Both);
            self.bricked = true;
        })
    }
    /// Inner logic of sender.
    fn send_inner(&mut self, msg: &S) -> Result<Result<(), SerializeError>, SendError> {
        let mut buf = self.buffer.take().expect("buffer should never be None");
        buf.clear();
        // Prepare length prefix that will hold a size of type [`u32`].
        buf.extend_from_slice(&[0; PREFIX_LEN]);
        let buf_ref: &mut [u8] = match postcard::to_extend(msg, buf) {
            Ok(b) => self.buffer.insert(b),
            Err(err) => {
                self.buffer = Some(Vec::with_capacity(DEFAULT_TX_BUFFER_SIZE));
                return Ok(Err(SerializeError::Postcard(err)));
            }
        };
        let len = buf_ref.len() - PREFIX_LEN;
        if len > MAX_MSG_LENGTH as usize {
            self.buffer = Some(Vec::with_capacity(DEFAULT_TX_BUFFER_SIZE));
            return Ok(Err(SerializeError::OutputTooLarge(len)));
        }
        // Prepend the length of the message to the message itself as a [`u32`].
        buf_ref[..PREFIX_LEN].copy_from_slice(&(len as u32).to_le_bytes());
        self.socket
            .lock()
            .expect("mutex should lock")
            .write_all(buf_ref)
            .map(Ok)
            .map_err(SendError::Write)
    }
}
