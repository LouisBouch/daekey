use std::{sync::{Arc, atomic::AtomicBool}, thread, time::Duration};

use libdae::{
    KeyCode,
    binder::Binder,
    input::{KeyAction, KeyState, Keybind},
    modifiers,
};

fn main() {
    let mut binder = Binder::new(2);

    binder.create_binding(
        Keybind::new(KeyCode::KEY_A, KeyState::Pressed, modifiers::LEFT_SHIFT),
        {
            let kep = KeyAction::new(KeyCode::KEY_C, KeyState::Pressed);
            let ker = KeyAction::new(KeyCode::KEY_C, KeyState::Released);
            let toggle = Arc::new(AtomicBool::new(false));
            let o = std::sync::atomic::Ordering::Relaxed;
            move |api| {
                toggle.store(!toggle.load(o), o);
                while toggle.load(o) {
                    api.send_key_actions(vec![kep, ker]);
                    api.send_key_tap(KeyCode::KEY_B, modifiers::LEFT_SHIFT);
                    thread::sleep(Duration::from_secs_f32(0.1));
                }
            }
        },
    );
    binder.launch();
}
