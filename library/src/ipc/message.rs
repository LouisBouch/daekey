//! Holds differents types of messages that can be sent across processes.

use serde::{Deserialize, Serialize};

//TODO: add possible message types.
/// A message from a connected process.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromProcess {
    /// Client will disconnect.
    Exit,
    /// Process wants to subscribe to a key.
    Subscribe,
}


/// A message from the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromDaemon {
}
