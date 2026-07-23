//! Holds api for the user to request command from the privileged process.
use std::{
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use evdev::KeyCode;

use crate::{
    input::{KeyAction, MouseAction},
    message::{self, AppliedModifiers, MsgToUInput},
};

/// Acts as an api for the user to request command from the privileged process.
pub struct Api {
    socket_to_worker: Option<UnixStream>,
    return_pile: Arc<Mutex<Vec<UnixStream>>>,
}
impl Drop for Api {
    fn drop(&mut self) {
        let Some(socket) = self.socket_to_worker.take() else {
            panic!("the bridge should have a valid socket");
        };
        self.return_pile
            .lock()
            .expect("lock should be sucessful")
            .push(socket);
    }
}
impl Api {
    // crossbeam channel to send message to a thread that will them pipe to the privileged process.
    pub fn new(socket_to_worker: UnixStream, return_pile: Arc<Mutex<Vec<UnixStream>>>) -> Self {
        Api {
            socket_to_worker: Some(socket_to_worker),
            return_pile,
        }
    }
    /// Send a list of key press/release to the compositor through the privileged process.
    pub fn send_key_actions(&self, actions: Vec<KeyAction>) {
        let mes = message::MsgToUInput::SendKeyActions(actions);
        self.send_msg(mes);
    }
    /// Send a key press followed by release to the compositor through the privileged process.
    pub fn send_key_tap(&self, key: KeyCode, modifiers: AppliedModifiers) {
        let mes = message::MsgToUInput::SendKeyTap(key, modifiers);
        self.send_msg(mes);
    }
    /// Send a key press for the mouse followed by release to the compositor through the privileged process.
    pub fn send_mouse_click(&self, key: KeyCode, modifiers: AppliedModifiers) {
        let mes = message::MsgToUInput::SendMouseClick(key, modifiers);
        self.send_msg(mes);
    }
    /// Send a relative mouse movement to the compositor through the privileged process.
    pub fn send_mouse_actions(&self, actions: Vec<MouseAction>) {
        let mes = message::MsgToUInput::SendMouseActions(actions);
        self.send_msg(mes);
    }
    /// Send a message to the privileged  process' UInput through a socket.
    fn send_msg(&self, mes: MsgToUInput) {
        let socket = self
            .socket_to_worker
            .as_ref()
            .expect("the bridge should have a valid socket");
        postcard::to_io(&message::MsgToWorker::UInputRequest(mes), socket)
            .expect("socket should successfully send message");
    }
}
