//! Handles uinput and virtual device.
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender};
use evdev::{AttributeSet, KeyCode, uinput::VirtualDevice};
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

pub fn launch_virtual_device() -> std::io::Result<UInputShare> {
    let mut key_set: AttributeSet<KeyCode> = AttributeSet::new();
    for i in 0..768 {
        key_set.insert(KeyCode::new(i));
    }
    let device = evdev::uinput::VirtualDevice::builder()?
        .name("virt_device")
        .with_keys(&key_set)?
        .build()?;
    let (sender, receiver) = crossbeam_channel::unbounded::<message::MsgToUInput>();
    let handle = std::thread::spawn(|| {
        uinput_loop(device, receiver);
    });
    Ok(UInputShare {
        handle,
        uinput_sender: sender,
    })
}
fn uinput_loop(mut device: VirtualDevice, receiver: Receiver<message::MsgToUInput>) {
    let type_key = evdev::EventType::KEY.0;
    // let lshift_p = evdev::InputEvent::new(type_key, KeyCode::KEY_LEFTSHIFT.0, 1);
    // let lshift_r = evdev::InputEvent::new(type_key, KeyCode::KEY_LEFTSHIFT.0, 0);
    let mut cur_modifiers: modifiers::Modifiers = modifiers::NONE;
    loop {
        match receiver.recv() {
            Ok(msg) => match msg {
                message::MsgToUInput::SendKeyActions(key_events) => {
                    let mut events = Vec::new();
                    for key_event in &key_events {
                        let state = key_event.state as i32;
                        let key = key_event.key;
                        events.push(evdev::InputEvent::new(type_key, key.code(), state));
                        let is_key_modi = modifiers::modifier_from_keycode(key);
                        if state == 0 {
                            // On release, remove the modifier.
                            cur_modifiers &= !is_key_modi;
                        } else if state == 1 {
                            // On press, add the modifier.
                            cur_modifiers |= is_key_modi;
                        };
                    }
                    match device.emit(&events) {
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
                        events.push(evdev::InputEvent::new(type_key, mod_code.code(), 1));
                    }
                    for mod_code in &mod_to_rm {
                        events.push(evdev::InputEvent::new(type_key, mod_code.code(), 0));
                    }
                    events.push(evdev::InputEvent::new(
                        type_key,
                        key.code(),
                        KeyState::Pressed as i32,
                    ));
                    events.push(evdev::InputEvent::new(
                        type_key,
                        key.code(),
                        KeyState::Released as i32,
                    ));
                    for mod_code in &mod_to_add {
                        events.push(evdev::InputEvent::new(type_key, mod_code.code(), 0));
                    }
                    for mod_code in &mod_to_rm {
                        events.push(evdev::InputEvent::new(type_key, mod_code.code(), 1));
                    }
                    match device.emit(&events) {
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
