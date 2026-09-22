//! Defines how the daemon will send input to the compositor.


/// Messages that can be sent to the uinput interface.
pub enum UInputMessage {
}

/// Holds necessary values to interface with uinput and talk back to the [`Router`].
pub struct UInput {
}
impl UInput {
    /// Launch the interface which will listen to message requests.
    pub fn launch(&self) {
    }
}
