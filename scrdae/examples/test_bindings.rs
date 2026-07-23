use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
    time::Duration,
};

use libdae::{
    AbsoluteAxisCode, AppliedModifiers, KeyCode, RelativeAxisCode,
    binder::Binder,
    input::{KeyAction, KeyState, Keybind, MouseAbsAction, MouseAction, MouseRelAction},
    modifiers::{self},
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
                    api.send_key_tap(
                        KeyCode::KEY_B,
                        AppliedModifiers::Exact(modifiers::LEFT_SHIFT),
                    );
                    api.send_key_tap(KeyCode::KEY_B, AppliedModifiers::Current);
                    thread::sleep(Duration::from_secs_f32(0.1));
                }
            }
        },
    );
    binder.create_binding(
        Keybind::new(KeyCode::KEY_T, KeyState::Pressed, modifiers::LEFT_SHIFT),
        {
            move |api| {
                api.send_key_tap(
                    KeyCode::KEY_T,
                    AppliedModifiers::Exact(modifiers::LEFT_SHIFT),
                );
            }
        },
    );
    binder.create_binding(
        Keybind::new(KeyCode::KEY_Q, KeyState::Pressed, modifiers::LEFT_SHIFT),
        {
            move |api| {
                api.send_mouse_actions(vec![MouseAction::Rel(MouseRelAction::new(
                    RelativeAxisCode::REL_X,
                    10,
                ))]);
            }
        },
    );
    binder.create_binding(
        Keybind::new(KeyCode::KEY_W, KeyState::Pressed, modifiers::LEFT_SHIFT),
        {
            move |api| {
                api.send_mouse_actions(vec![
                    MouseAction::Abs(MouseAbsAction::new(AbsoluteAxisCode::ABS_X, 100)),
                    MouseAction::Abs(MouseAbsAction::new(AbsoluteAxisCode::ABS_Y, 100)),
                ]);
            }
        },
    );
    binder.create_binding(
        Keybind::new(KeyCode::KEY_SPACE, KeyState::Pressed, modifiers::NONE),
        {
            move |api| {
                api.send_mouse_click(KeyCode::BTN_LEFT, AppliedModifiers::Exact(modifiers::NONE));
            }
        },
    );
    binder.set_exit_key(Keybind::new(
        KeyCode::KEY_PAUSE,
        KeyState::Pressed,
        modifiers::RIGHT_SHIFT,
    ));
    binder.launch();
}
