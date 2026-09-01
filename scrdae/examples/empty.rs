use std::time::Duration;

use libdae::{
    KeyCode, app,
    configs::{self, Configs},
    input::{KeyState, Keybind},
    modifiers::{self},
};

fn main() {
    let mut binder = Configs::new(2);
    binder.create_binding(
        Keybind::new(
            KeyCode::KEY_PAUSE,
            KeyState::Pressed,
            modifiers::RIGHT_SHIFT,
        ),
        configs::Action::Exit,
    );
    binder.set_rel_mouse_polling_interval(Duration::from_millis(1000));
    app::App::new(binder).launch();
}
