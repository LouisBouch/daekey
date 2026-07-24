use libdae::{
    KeyCode,
    binder::Binder,
    input::{KeyState, Keybind},
    modifiers::{self},
};

fn main() {
    let mut binder = Binder::new(2);
    binder.set_exit_key(Keybind::new(
        KeyCode::KEY_PAUSE,
        KeyState::Pressed,
        modifiers::RIGHT_SHIFT,
    ));
    binder.launch();
}
