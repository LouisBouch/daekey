//! Holds api logic for the user to request commands from the privileged process.
use std::{
    ops::Deref,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
};

use evdev::KeyCode;

use crate::{
    compositor_interface::{CompositorInterface, CursorFetchErr, Point, ScreenInfo},
    input::{KeyAction, MouseAction},
    message::{self, AppliedModifiers, MsgToUInput},
};

/// Acts as an api for the user to request command from the privileged process.
pub struct Api {
    socket_to_worker: Option<UnixStream>,
    cmp_intf: CompositorInterface,
}
impl Api {
    // crossbeam channel to send message to a thread that will them pipe to the privileged process.
    pub(crate) fn new(socket_to_worker: UnixStream, cmp_intf: CompositorInterface) -> Self {
        Api {
            socket_to_worker: Some(socket_to_worker),
            cmp_intf,
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
    /// Get the absolute mouse position.
    pub fn get_abs_mouse_position(&self) -> Result<Point, CursorFetchErr> {
        self.cmp_intf.req_abs_cursor_pos()
    }
    /// Get info about the screens .
    pub fn get_screen_info(&self) -> Vec<ScreenInfo> {
        self.cmp_intf.req_screen_info()
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
/// Holds an Api and knows how to return it.
pub struct ApiHolder {
    api: Option<Api>,
    return_pile: Arc<Mutex<Vec<Api>>>,
}
impl ApiHolder {
    // Wraps an [`Api`] and puts it back on the pile when it is done.
    pub fn new(api: Api, return_pile: Arc<Mutex<Vec<Api>>>) -> Self {
        ApiHolder {
            api: Some(api),
            return_pile,
        }
    }
}
impl Deref for ApiHolder {
    type Target = Api;

    fn deref(&self) -> &Self::Target {
        self.api
            .as_ref()
            .expect("the api should not be removed until the 'ApiHolder is dropped")
    }
}
impl Drop for ApiHolder {
    fn drop(&mut self) {
        let Some(api) = self.api.take() else {
            panic!("the holder should have a valid api");
        };
        self.return_pile
            .lock()
            .expect("lock should be sucessful")
            .push(api);
    }
}
