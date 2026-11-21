//! Defines senders and receivers for interprocess communication.


/// Length of the size prefix added to messages.
const PREFIX_LEN: usize = size_of::<u32>();
/// Maximum size of message to send, in bytes.
/// NOTE: Won't work on 16bit architecture.
const MAX_MSG_LENGTH: u32 = 10_000_000;

pub mod tx;
pub mod rx;

