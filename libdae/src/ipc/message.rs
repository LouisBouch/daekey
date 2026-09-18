//! Holds differents types of messages that can be sent across processes.

use serde::{Deserialize, Serialize};

/// A message from a connected process.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromProcess {
}


/// A message from the daemon.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum FromDaemon {
}
