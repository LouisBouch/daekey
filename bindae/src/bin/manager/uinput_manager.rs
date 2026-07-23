//! Handles uinput and virtual device.
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, KeyCode, PropType, RelativeAxisCode, UinputAbsSetup,
    uinput::VirtualDevice,
};
use libdae::{
    display_monitor::ScreenSpace,
    input::KeyState,
    message,
    modifiers::{self, Modifiers},
};

pub struct UInputShare {
    handle: JoinHandle<()>,
    uinput_sender: Sender<message::MsgToUInput>,
}
impl UInputShare {
    pub fn join(self) {
        self.handle
            .join()
            .expect("input thread should join successfully");
    }
    pub fn uinput_sender(&self) -> &crossbeam_channel::Sender<message::MsgToUInput> {
        &self.uinput_sender
    }
}

pub fn launch_uinput_listener(screen_space: &ScreenSpace) -> std::io::Result<UInputShare> {
    let mut kbd_key_set: AttributeSet<KeyCode> = AttributeSet::new();
    //reference: https://docs.rs/evdev/latest/src/evdev/scancodes.rs.html
    for i in (0..=248).chain(0x160..=0x21e).chain(0x230..=0x27a) {
        kbd_key_set.insert(KeyCode::new(i));
    }
    let virt_kbd = evdev::uinput::VirtualDevice::builder()?
        .name("virt_keyboard")
        .with_keys(&kbd_key_set)?
        .build()?;

    let mut mouse_key_set_rel: AttributeSet<KeyCode> = AttributeSet::new();
    //reference: https://docs.rs/evdev/latest/src/evdev/scancodes.rs.html
    for i in 0x100..=0x151 {
        mouse_key_set_rel.insert(KeyCode::new(i));
    }
    let mut mouse_rel_axis: AttributeSet<RelativeAxisCode> = AttributeSet::new();
    mouse_rel_axis.insert(RelativeAxisCode::REL_X);
    mouse_rel_axis.insert(RelativeAxisCode::REL_Y);
    mouse_rel_axis.insert(RelativeAxisCode::REL_WHEEL);
    let virt_mouse_rel = evdev::uinput::VirtualDevice::builder()?
        .name("virt_mouse_rel")
        .with_keys(&mouse_key_set_rel)?
        .with_relative_axes(&mouse_rel_axis)?
        .build()?;

    let range = screen_space.range();
    let info_x = AbsInfo::new(0, range.0[0], range.1[0] - 1, 0, 0, 1);
    let info_y = AbsInfo::new(0, range.0[1], range.1[1] - 1, 0, 0, 1);
    let mut mouse_key_set_abs: AttributeSet<KeyCode> = AttributeSet::new();
    mouse_key_set_abs.insert(KeyCode::new(0x140)); //tablet pen
    let mut abs_prop: AttributeSet<PropType> = AttributeSet::new();
    abs_prop.insert(evdev::PropType::DIRECT);
    let virt_mouse_abs = evdev::uinput::VirtualDevice::builder()?
        .name("virt_mouse_abs")
        .with_keys(&mouse_key_set_abs)?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, info_x))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, info_y))?
        .build()?;

    let (sender, receiver) = crossbeam_channel::unbounded::<message::MsgToUInput>();
    let handle = std::thread::spawn(|| {
        uinput_loop(virt_kbd, virt_mouse_rel, virt_mouse_abs, receiver);
    });
    Ok(UInputShare {
        handle,
        uinput_sender: sender,
    })
}
fn uinput_loop(
    mut virt_kbd: VirtualDevice,
    mut virt_mouse_rel: VirtualDevice,
    mut virt_mouse_abs: VirtualDevice,
    receiver: Receiver<message::MsgToUInput>,
) {
    let type_key = evdev::EventType::KEY.0;
    let type_mouse_rel = evdev::EventType::RELATIVE.0;
    let type_mouse_abs = evdev::EventType::ABSOLUTE.0;
    let mut cur_modifiers: modifiers::Modifiers = modifiers::NONE;
    loop {
        match receiver.recv() {
            Ok(msg) => match msg {
                message::MsgToUInput::SendKeyActions(key_actions) => {
                    let mut events = Vec::new();
                    for key_action in &key_actions {
                        let state = key_action.state as i32;
                        let key = key_action.key;
                        events.push(evdev::InputEvent::new_now(type_key, key.code(), state));
                        let is_key_modi = modifiers::modifier_from_keycode(key);
                        if state == 0 {
                            // On release, remove the modifier.
                            cur_modifiers &= !is_key_modi;
                        } else if state == 1 {
                            // On press, add the modifier.
                            cur_modifiers |= is_key_modi;
                        };
                    }
                    match virt_kbd.emit(&events) {
                        Err(e) => eprintln!("failed to send keys '{key_actions:?}': {e}"),
                        _ => (),
                    }
                }
                message::MsgToUInput::SendKeyTap(key_code, req_modifiers) => {
                    send_full_motion(key_code, &mut virt_kbd, cur_modifiers, req_modifiers, false);
                }
                message::MsgToUInput::SendMouseActions(mouse_actions) => {
                    let mut rel_events = Vec::new();
                    let mut abs_events = Vec::new();

                    for mouse_action in &mouse_actions {
                        match mouse_action {
                            libdae::input::MouseAction::Abs(mouse_abs_action) => {
                                // Add these in first to ensure the kernel accepts to move the cursor to the
                                // same position twice.
                                if abs_events.is_empty() {
                                    abs_events.push(evdev::InputEvent::new_now(
                                        type_mouse_abs,
                                        evdev::AbsoluteAxisCode::ABS_X.0,
                                        -1,
                                    ));
                                    abs_events.push(evdev::InputEvent::new_now(
                                        type_mouse_abs,
                                        evdev::AbsoluteAxisCode::ABS_Y.0,
                                        -1,
                                    ));
                                }
                                let axis = mouse_abs_action.axis;
                                let pos = mouse_abs_action.pos;
                                abs_events.push(evdev::InputEvent::new_now(
                                    type_mouse_abs,
                                    axis.0,
                                    pos,
                                ));
                            }
                            libdae::input::MouseAction::Rel(mouse_rel_action) => {
                                let axis = mouse_rel_action.axis;
                                let value = mouse_rel_action.value;
                                rel_events.push(evdev::InputEvent::new_now(
                                    type_mouse_rel,
                                    axis.0,
                                    value,
                                ));
                            }
                            libdae::input::MouseAction::Key(key_action) => {
                                let state = key_action.state as i32;
                                let key = key_action.key;
                                rel_events.push(evdev::InputEvent::new_now(
                                    type_key,
                                    key.code(),
                                    state,
                                ));
                            }
                        }
                    }
                    if !rel_events.is_empty() {
                        match virt_mouse_rel.emit(&rel_events) {
                            Err(e) => {
                                eprintln!("failed to send mouse actions '{rel_events:?}': {e}")
                            }
                            _ => (),
                        }
                    }
                    if !abs_events.is_empty() {
                        match virt_mouse_abs.emit(&abs_events) {
                            Err(e) => {
                                eprintln!("failed to send mouse actions '{abs_events:?}': {e}")
                            }
                            _ => (),
                        }
                    }
                }
                message::MsgToUInput::SendMouseClick(key_code, req_modifiers) => {
                    send_full_motion(
                        key_code,
                        &mut virt_mouse_rel,
                        cur_modifiers,
                        req_modifiers,
                        true,
                    );
                }
            },
            Err(e) => eprintln!("failed to receive message: {}", e),
        }
    }
}
/// Sends a full key motion using specified modifiers. Full motion implies a down and up action.
fn send_full_motion(
    key_code: KeyCode,
    virt_dev: &mut VirtualDevice,
    cur_modifiers: Modifiers,
    req_modifiers: message::AppliedModifiers,
    // Whether to send the motion in a single emit or two individual ones.
    send_separate: bool,
) {
    let type_key = evdev::EventType::KEY.0;
    let mut events = Vec::new();
    // Modifiers to add or remove for the duration of the keypress.
    // Undone after key is sent.
    let (mod_to_add, mod_to_rm) = match req_modifiers {
        libdae::AppliedModifiers::Current => (Vec::new(), Vec::new()),
        libdae::AppliedModifiers::Exact(req_modifiers) => (
            modifiers::keycodes_from_modifiers(!cur_modifiers & req_modifiers),
            modifiers::keycodes_from_modifiers(cur_modifiers & !req_modifiers),
        ),
    };
    for mod_code in &mod_to_add {
        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 1));
    }
    for mod_code in &mod_to_rm {
        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 0));
    }
    events.push(evdev::InputEvent::new_now(
        type_key,
        key_code.code(),
        KeyState::Pressed as i32,
    ));
    if send_separate {
        match virt_dev.emit(&events) {
            Err(e) => eprintln!("failed to tap key '{events:?}': {e}"),
            _ => (),
        }
        events.clear();
    }
    events.push(evdev::InputEvent::new_now(
        type_key,
        key_code.code(),
        KeyState::Released as i32,
    ));
    for mod_code in &mod_to_add {
        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 0));
    }
    for mod_code in &mod_to_rm {
        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 1));
    }
    match virt_dev.emit(&events) {
        Err(e) => eprintln!("failed to tap key '{key_code:?}': {e}"),
        _ => (),
    }
}
