//! Handles uinput and virtual device.
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, KeyCode, PropType, RelativeAxisCode, UinputAbsSetup,
    uinput::VirtualDevice,
};
use libdae::{keys::KeyState, message, modifiers};

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

pub fn launch_uinput_listener() -> std::io::Result<UInputShare> {
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

    let info_xy = AbsInfo::new(0, 0, i32::MAX as i32 / 2, 0, 0, 1);
    let mut mouse_key_set_abs: AttributeSet<KeyCode> = AttributeSet::new();
    mouse_key_set_abs.insert(KeyCode::new(0x140)); //tablet pen
    let mut abs_prop: AttributeSet<PropType> = AttributeSet::new();
    abs_prop.insert(evdev::PropType::DIRECT);
    let virt_mouse_abs = evdev::uinput::VirtualDevice::builder()?
        .name("virt_mouse_abs")
        .with_keys(&mouse_key_set_abs)?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, info_xy))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, info_xy))?
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
                message::MsgToUInput::SendKeyActions(key_events) => {
                    let mut events = Vec::new();
                    for key_event in &key_events {
                        let state = key_event.state as i32;
                        let key = key_event.key;
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
                        Err(e) => eprintln!("failed to send keys '{key_events:?}': {e}"),
                        _ => {
                            // println!("sent series: {events:?}");
                        }
                    }
                }
                message::MsgToUInput::SendKeyTap(key, req_modi) => {
                    let mut events = Vec::new();
                    // Modifiers to add or remove for the duration of the keypress.
                    // Undone after key is sent.
                    let mod_to_add = modifiers::keycodes_from_modifiers(!cur_modifiers & req_modi);
                    let mod_to_rm = modifiers::keycodes_from_modifiers(cur_modifiers & !req_modi);
                    for mod_code in &mod_to_add {
                        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 1));
                    }
                    for mod_code in &mod_to_rm {
                        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 0));
                    }
                    events.push(evdev::InputEvent::new_now(
                        type_key,
                        key.code(),
                        KeyState::Pressed as i32,
                    ));
                    events.push(evdev::InputEvent::new_now(
                        type_key,
                        key.code(),
                        KeyState::Released as i32,
                    ));
                    for mod_code in &mod_to_add {
                        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 0));
                    }
                    for mod_code in &mod_to_rm {
                        events.push(evdev::InputEvent::new_now(type_key, mod_code.code(), 1));
                    }
                    // virt_mouse_rel.emit(&[evdev::InputEvent::new_now(type_mouse_rel, evdev::RelativeAxisCode::REL_X.0, 1)]);
                    virt_mouse_abs
                        .emit(&[
                            evdev::InputEvent::new_now(type_key, 0x140, 0),
                            evdev::InputEvent::new_now(
                                type_mouse_abs,
                                evdev::AbsoluteAxisCode::ABS_X.0,
                                -1,
                            ),
                            evdev::InputEvent::new_now(
                                type_mouse_abs,
                                evdev::AbsoluteAxisCode::ABS_Y.0,
                                -1,
                            ),
                            evdev::InputEvent::new_now(
                                type_mouse_abs,
                                evdev::AbsoluteAxisCode::ABS_X.0,
                                0,
                            ),
                            evdev::InputEvent::new_now(
                                type_mouse_abs,
                                evdev::AbsoluteAxisCode::ABS_Y.0,
                                0,
                            ),
                        ])
                        .unwrap();
                    // virt_mouse_abs.emit(&[ evdev::InputEvent::new_now(type_key, 0x140,0)]);
                    match virt_kbd.emit(&events) {
                        Err(e) => eprintln!("failed to tap key '{key:?}': {e}"),
                        _ => {
                            // println!("sent tap: {events:?}");
                        }
                    }
                }
            },
            Err(e) => eprintln!("failed to receive message: {}", e),
        }
    }
}
