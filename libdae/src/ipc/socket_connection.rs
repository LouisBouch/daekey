//! Handles connection between daemon and process using a socket.

use std::{error::Error, fmt::Display, io::Write, marker::PhantomData, os::unix::net::UnixStream};

use serde::{Serialize, de::DeserializeOwned};

const DEFAULT_TX_BUFFER_SIZE: usize = 1024;
const DEFAULT_RX_SCRATCH_BUFFER_SIZE: usize = 1024 * 128;
const DEFAULT_RX_READ_BUFFER_SIZE: usize = 1024 * 8;

/// Represents an error while trying to send a message.
#[derive(Debug)]
pub enum SendError {
    /// An error from [`postcard`]
    Postcard(postcard::Error),
    /// An error from [`std::io`]
    Io(std::io::Error),
}
impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Postcard(error) => {
                write!(f, "Error while serializing message with postcard: {error}")
            }
            SendError::Io(error) => {
                write!(f, "Error while sending message over socket: {error}")
            }
        }
    }
}
impl Error for SendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendError::Postcard(error) => Some(error),
            SendError::Io(error) => Some(error),
        }
    }
}
/// A connection that sends messages to a connected process.
pub struct ConnectionTx<S: Serialize> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
    /// Generic type has to be held somewhere.
    _t: PhantomData<S>,
    /// Used to serialzie data into.
    _buffer: Option<Vec<u8>>,
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
            _buffer: Some(Vec::with_capacity(DEFAULT_TX_BUFFER_SIZE)),
        }
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
            socket: socket,
            _t: PhantomData,
            _buffer: Some(Vec::with_capacity(starting_buffer_capacity)),
        }
    }
    /// Send message over the socket.
    pub fn send(&mut self, msg: S) -> Result<(), SendError> {
        let e = "_buffer should never be None";
        self._buffer.as_mut().expect(e).clear();
        match postcard::to_extend(&msg, self._buffer.take().expect(e)) {
            Ok(b) => self._buffer = Some(b),
            Err(e) => {
                self._buffer = Some(Vec::with_capacity(1024));
                return Err(SendError::Postcard(e));
            }
        }
        self.socket
            .write_all(&self._buffer.as_ref().expect(e))
            .map_err(SendError::Io)?;
        Ok(())
    }
}

/// A connection that receives messages from a connected process.
pub struct ConnectionRx<R: DeserializeOwned> {
    /// Socket connecting a process to the daemon wrapped by a buffer reader.
    socket: std::io::BufReader<UnixStream>,
    /// Buffer for the receiver.
    scratch_buffer: Vec<u8>,
    /// Generic type has to be held somewhere.
    _t: PhantomData<R>,
}
impl<R: DeserializeOwned> ConnectionRx<R> {
    /// Create a new connection receiver. This can be used to receive data from a socket.
    /// Default buffer size for receiver is 1024 * 128 bytes.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    pub fn new(socket: UnixStream) -> Self {
        Self {
            socket: std::io::BufReader::with_capacity(DEFAULT_RX_READ_BUFFER_SIZE, socket),
            scratch_buffer: vec![0; DEFAULT_RX_SCRATCH_BUFFER_SIZE],
            _t: PhantomData,
        }
    }
    /// Create a new connection receiver given a buffer size. This can be used to receive data from a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    /// * `scratch_buffer_size` - The size the scratch buffer uses for deserializing data into.
    /// * `read_buffer_size` - The size the read buffer used for reading bytes with a syscall.
    pub fn with_buffer_size(
        socket: UnixStream,
        scratch_buffer_size: usize,
        read_buffer_size: usize,
    ) -> Self {
        Self {
            socket: std::io::BufReader::with_capacity(read_buffer_size, socket),
            scratch_buffer: vec![0; scratch_buffer_size],
            _t: PhantomData,
        }
    }
    /// Block while waiting for a new message.
    pub fn recv(&mut self) -> postcard::Result<R> {
        Ok(postcard::from_io((&mut self.socket, &mut self.scratch_buffer))?.0)
    }
}

// TODO: A connection that can both send and receive messages?
