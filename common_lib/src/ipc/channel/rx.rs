//! Receives messages by deserializing and using a wrapped socket.

use std::{error::Error, fmt::Display, io::Read, marker::PhantomData, os::unix::net::UnixStream};

use serde::de::DeserializeOwned;

use super::MAX_MSG_LENGTH;
use super::PREFIX_LEN;

const DEFAULT_RX_SCRATCH_BUFFER_SIZE: usize = 1024 * 8;

/// Represents an error while trying to send a message.
#[derive(Debug)]
pub enum ReceiveError {
    /// Failed to read from the socket.
    Read(std::io::Error),
    /// Failed to deserialize message with [`postcard`].
    Deserialize(postcard::Error),
    /// The message received is too big.
    MessageTooLarge { size: usize },
    /// The receiver is bricked and cannot receive other messages.
    Bricked,
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
            ReceiveError::MessageTooLarge { size } => {
                write!(
                    f,
                    "failed to receive message over socket, message is too large. max: {} bytes, message size: {} bytes. shutting down socket",
                    u32::MAX,
                    size
                )
            }
            ReceiveError::Bricked => {
                write!(
                    f,
                    "cannot receive message, the receiver has already encountered a fatal error"
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
            ReceiveError::MessageTooLarge { size: _ } => None,
            ReceiveError::Bricked => None,
        }
    }
}
/// Error when deserializing the received message.
#[derive(Debug)]
pub enum DeserializeError {
    /// Postcard failed to deserialize the message.
    Postcard(postcard::Error),
    /// Bytes remained after reading the message.
    TrailingBytes(usize),
}
impl Display for DeserializeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeserializeError::Postcard(_) => {
                write!(f, "postcard failed to deserialize message")
            }
            DeserializeError::TrailingBytes(size) => {
                write!(f, "{size} byte(s) were not deserialized and discarded")
            }
        }
    }
}
impl Error for DeserializeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DeserializeError::Postcard(error) => Some(error),
            DeserializeError::TrailingBytes(_) => None,
        }
    }
}

/// A connection that receives messages from a connected process.
pub struct Rx<R: DeserializeOwned> {
    /// Socket connecting the processes.
    socket: UnixStream,
    /// Buffer for the receiver.
    scratch_buffer: Vec<u8>,
    /// Whether the receiver has encountered a fatal error.
    bricked: bool,
    /// Generic type has to be held somewhere.
    _t: PhantomData<R>,
}
impl<R: DeserializeOwned> Rx<R> {
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
            bricked: false,
            _t: PhantomData,
        }
    }
    /// Block while waiting for a new message.
    pub fn recv(&mut self) -> Result<Result<R, DeserializeError>, ReceiveError> {
        if self.bricked {
            return Err(ReceiveError::Bricked);
        }
        self.recv_inner().inspect_err(|_| {
            let _ = self.socket.shutdown(std::net::Shutdown::Both);
            self.bricked = true;
        })
    }
    /// The inner logic of the receiver.
    fn recv_inner(&mut self) -> Result<Result<R, DeserializeError>, ReceiveError> {
        let socket = &mut self.socket;
        let mut len_arr = [0; PREFIX_LEN];
        socket
            .read_exact(&mut len_arr)
            .map_err(ReceiveError::Read)?;
        let len = u32::from_le_bytes(len_arr);
        if len > MAX_MSG_LENGTH {
            // Won't retrieve humongous message, socket is now bricked.
            return Err(ReceiveError::MessageTooLarge { size: len as usize });
        }
        let scratch = &mut self.scratch_buffer;
        scratch.resize(len as usize, 0);
        socket.read_exact(scratch).map_err(ReceiveError::Read)?;
        Ok(match postcard::take_from_bytes(scratch) {
            Ok((res, tail)) if tail.is_empty() => Ok(res),
            Ok((_, tail)) => Err(DeserializeError::TrailingBytes(tail.len())),
            Err(e) => Err(DeserializeError::Postcard(e)),
        })
    }
}
