//! Defines how the daemon will receive inputs from the compositor.

/// Messages that can be sent to the input interface.
pub enum InputMessage {
}

/// Holds necessary values to interface with input devices and talk back to the [`Router`].
pub struct Input {
}
impl Input {
    /// Launch the interface which will listen to input devices and message requests.
    pub fn launch(&self) {
    }
}
