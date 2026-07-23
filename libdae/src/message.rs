//! Contains list of possible messages that will be sent across and within processes.

use std::collections::HashSet;

use crate::{input::{KeyAction, Keybind, MouseAction}, modifiers::Modifiers};
use crossbeam_channel::Sender;
use evdev::KeyCode;
use serde::{Deserialize, Serialize};


#[derive(Serialize, Deserialize, Debug)]
/// Message that comes from the main unprivileged process to the privileged one.
pub enum MsgToPriv {
    Quit,
}
/// Message that comes from the child privileged process to the unprivileged core.
#[derive(Serialize, Deserialize, Debug)]
pub enum MsgToCore {
}
/// Message sent by closure to privileged sockets.
#[derive(Serialize, Deserialize, Debug)]
pub enum MsgToWorker {
    UInputRequest(MsgToUInput),
}
/// Decides how the modifiers are applied.
#[derive(Serialize, Deserialize, Debug)]
pub enum AppliedModifiers {
    /// Use the modifiers currently active.
    Current,
    /// Ensure an exact set of modifiers is active.
    Exact(Modifiers),
}
/// Mesasge to UInput thread.
#[derive(Serialize, Deserialize, Debug)]
pub enum MsgToUInput {
    SendKeyActions(Vec<KeyAction>),
    SendKeyTap(KeyCode, AppliedModifiers),
    SendMouseActions(Vec<MouseAction>),
    SendMouseClick(KeyCode, AppliedModifiers),
}

/// Mesasge to Input thread.
#[derive(Serialize, Deserialize, Debug)]
pub enum MsgToInput {
    ChangeBindings(HashSet<Keybind>)
}

/// Message sent by privileged socket to closure.
pub enum WorkerReply {
}

/// Sends messages through channels. Assumes receiving channels are always connected.
///
/// # Arguments
///
/// * `tx` - The channel used to transmit the message.
/// * `command` - The command to send through the channel.
pub fn send_msg<T>(tx: &Sender<T>, command: T) {
    let err_mes = "the receiver should not be disconnected";
    tx.send(command).expect(err_mes);
}
