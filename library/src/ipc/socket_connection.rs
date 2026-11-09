//! Handles connection between daemon and process using a socket.

use std::{
    error::Error,
    fmt::Display,
    io::{BufRead, Read, Write},
    marker::PhantomData,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use serde::{Serialize, de::DeserializeOwned};

const DEFAULT_TX_BUFFER_SIZE: usize = 1024;
const DEFAULT_RX_SCRATCH_BUFFER_SIZE: usize = 1024 * 8;
/// Length of the size prefix added to messages.
const PREFIX_LEN: usize = size_of::<u32>();
/// Maximum size of message to send, in bytes.
/// NOTE: Won't work on 16bit architecture.
const MAX_SEND_LENGTH: u32 = 10_000_000;

/// Represents an error while trying to send a message.
#[derive(Debug)]
pub enum ReceiveError {
    /// Failed to read from the socket.
    /// Fatal.
    Read(std::io::Error),
    /// Failed to deserialize message with [`postcard`].
    /// Non Fatal.
    Deserialize(postcard::Error),
    /// The message to receive is too big.
    ///
    /// * `usize` - Length of the message that was supposed to be received.
    /// Fatal.
    MessageSize(usize),
}
impl Display for ReceiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReceiveError::Read(_) => {
                write!(f, "failed to read from socket")
            }
            ReceiveError::Deserialize(_) => {
                write!(f, "failed to deserialize message")
            }
            ReceiveError::MessageSize(size) => {
                write!(
                    f,
                    "failed to receive message over socket, message is too large. max: {} bytes, message size: {} bytes. shutting down socket",
                    u32::MAX,
                    size
                )
            }
        }
    }
}
impl Error for ReceiveError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ReceiveError::Read(error) => Some(error),
            ReceiveError::Deserialize(error) => Some(error),
            ReceiveError::MessageSize(_) => None,
        }
    }
}
/// Represents an error while trying to send a message.
#[derive(Debug)]
pub enum SendError {
    /// Failed to serialize a value with [`postcard`].
    Serialize(postcard::Error),
    /// Failed to write to socket.
    Write(std::io::Error),
    /// The message to send is too big.
    ///
    /// * `usize` - Length of the message that was supposed to send, in bytes.
    /// Non Fatal.
    MessageSize(usize),
}
impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendError::Serialize(_) => {
                write!(f, "failed to serialize message")
            }
            SendError::Write(_) => {
                write!(f, "failed to send message over socket")
            }
            SendError::MessageSize(size) => {
                write!(
                    f,
                    "failed to send message over socket, message is too large. max: {} bytes, message size: {} bytes",
                    u32::MAX,
                    size
                )
            }
        }
    }
}
impl Error for SendError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SendError::Serialize(error) => Some(error),
            SendError::Write(error) => Some(error),
            SendError::MessageSize(_) => None,
        }
    }
}

/// A connection that sends messages to a connected process.
#[derive(Clone)]
pub struct ConnectionTx<S: Serialize> {
    /// Socket connecting a process to the daemon.
    socket: Arc<Mutex<UnixStream>>,
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
            _t: PhantomData,
            _buffer: Some(Vec::with_capacity(starting_buffer_capacity)),
        }
    }
    /// Send message over the socket.
    pub fn send(&mut self, msg: S) -> Result<(), SendError> {
        let mut buf = self._buffer.take().expect("_buffer should never be None");
        buf.clear();
        // Prepare length prefix that will hold a size of type [`u32`].
        buf.extend_from_slice(&[0; PREFIX_LEN]);
        let buf_ref: &mut [u8] = match postcard::to_extend(&msg, buf) {
            Ok(b) => self._buffer.insert(b),
            Err(err) => {
                self._buffer = Some(Vec::with_capacity(DEFAULT_TX_BUFFER_SIZE));
                return Err(SendError::Serialize(err));
            }
        };
        let len = buf_ref.len() - PREFIX_LEN;
        if len > MAX_SEND_LENGTH as usize {
            return Err(SendError::MessageSize(len));
        }
        // Prepend the length of the message to the message itself as a [`u32`].
        buf_ref[..PREFIX_LEN].copy_from_slice(&(len as u32).to_le_bytes());
        self.socket
            .lock()
            .expect("mutex should lock")
            .write_all(buf_ref)
            .map_err(SendError::Write)
    }
}

/// A connection that receives messages from a connected process.
pub struct ConnectionRx<R: DeserializeOwned> {
    /// Socket connecting a process to the daemon.
    socket: UnixStream,
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
        Self::with_buffer_size(socket, DEFAULT_RX_SCRATCH_BUFFER_SIZE)
    }
    /// Create a new connection receiver given a buffer size. This can be used to receive data from a socket.
    ///
    /// # Parameters
    ///
    /// * `socket` - The socket used for inter process communication.
    /// * `scratch_buffer_size` - The size the scratch buffer uses for deserializing data into.
    pub fn with_buffer_size(socket: UnixStream, scratch_buffer_size: usize) -> Self {
        Self {
            socket,
            scratch_buffer: vec![0; scratch_buffer_size],
            _t: PhantomData,
        }
    }
    /// Block while waiting for a new message.
    pub fn recv(&mut self) -> Result<R, ReceiveError> {
        let socket = &mut self.socket;
        let mut len_arr = [0; PREFIX_LEN];
        socket
            .read_exact(&mut len_arr)
            .map_err(ReceiveError::Read)?;
        let len = u32::from_le_bytes(len_arr);
        if len > MAX_SEND_LENGTH {
            // Can't retrieve message, socket is now bricked so close it.
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
            return Err(ReceiveError::MessageSize(len as usize));
        }
        let scratch = &mut self.scratch_buffer;
        scratch.resize(len as usize, 0);
        socket
            .read_exact(scratch)
            .map_err(ReceiveError::Read)?;
        postcard::from_bytes(scratch).map_err(ReceiveError::Deserialize)
    }
}
