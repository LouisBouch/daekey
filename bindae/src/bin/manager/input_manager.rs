//! Handles input devices..
use std::{
    collections::HashSet,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::net::UnixStream,
    },
    path::PathBuf,
    thread::JoinHandle,
};

pub struct InputShare {
    handle: JoinHandle<()>,
}
impl InputShare {
    pub fn join(self) {
        self.handle
            .join()
            .expect("input thread should join successfully");
    }
}

use crossbeam_channel::Sender;
use evdev::{Device, KeyCode};
use libdae::{
    keys::{KeyAction, KeyState, Keybind},
    message, modifiers,
};
use nix::poll::{PollFlags, PollTimeout, poll};

pub fn launch_input_listener(
    // bindings: HashSet<Keybind>,
    input_socket: UnixStream,
    uinput_channel: Sender<message::MsgToUInput>,
) -> std::io::Result<InputShare> {
    let dir = std::path::Path::new("/dev/input/by-path/");
    let files = std::fs::read_dir(dir)?;
    let mut device_paths: Vec<std::path::PathBuf> = Vec::new();
    for file in files {
        let file = file.expect("file should be valid directory entry");
        let name = file
            .file_name()
            .into_string()
            .expect("filename should not contain invalid characters");
        if name.contains("kbd") {
            device_paths.push(std::fs::canonicalize(
                dir.join(std::fs::read_link(dir.join(name.clone()))?),
            )?);
        }
    }
    // Ensure path uniqueness.
    {
        let mut set: HashSet<PathBuf> = HashSet::new();
        device_paths.retain(|v| set.insert(v.clone()));
    }

    let mut device_list = Vec::new();
    for device_path in device_paths {
        let mut dev = evdev::Device::open(device_path.clone())?;
        // Wait for all keys to be released before grabbing the device.
        loop {
            if dev.get_key_state()?.iter().len() == 0 {
                dev.grab()?;
                if dev.get_key_state()?.iter().len() == 0 {
                    break;
                } else {
                    dev.ungrab()?;
                    println!(
                        "Releasing grab: Key was pressed after checking that all keys were released but before device was grabbed."
                    );
                    println!(
                        "If key was released before ungrab happened, the key will be stuck pressed down until pressed and released again. (very very very very very highly unlikely. Requires microsecond precision.)"
                    );
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        // for code in dev.get_key_state()?.iter() {
        // let key_event = libdae::KeyAction::new(code, libdae::KeyState::Released);
        // message::send_msg(
        //     &uinput_channel,
        //     message::MsgToUInput::SendKeyActions(vec![key_event]),
        // );
        // }
        device_list.push(dev);
    }
    let handle = std::thread::spawn(move || {
        input_loop(device_list, uinput_channel, input_socket).expect("input loop should not fail");
    });
    Ok(InputShare { handle })
}
fn input_loop(
    mut device_list: Vec<Device>,
    uinput_channel: Sender<message::MsgToUInput>,
    input_socket: UnixStream,
) -> std::io::Result<()> {
    let mut bindings: HashSet<Keybind> = HashSet::new();
    // Listen to devices.
    let mut polling_fds: Vec<_> = device_list
        .iter()
        .map(|v| unsafe {
            nix::poll::PollFd::new(BorrowedFd::borrow_raw(v.as_raw_fd()), PollFlags::POLLIN)
        })
        .collect();
    unsafe {
        polling_fds.push(nix::poll::PollFd::new(
            BorrowedFd::borrow_raw(input_socket.as_raw_fd()),
            PollFlags::POLLIN,
        ))
    };
    let mut cur_modifiers_per_dev: Vec<modifiers::Modifiers> = vec![modifiers::NONE; 2];

    loop {
        if poll(&mut polling_fds, PollTimeout::NONE).is_err() {
            break;
        }

        let sf_exp = "status flag should be valid";
        // Find which device triggered.
        for poll_fd_id in 0..polling_fds.len() {
            if !polling_fds[poll_fd_id].any().expect(sf_exp) {
                continue;
            }
            // Message for input and not from a device.
            // This channel is exclusively used to send bindings.
            if poll_fd_id == polling_fds.len() - 1 {
                bindings = postcard::from_io((&input_socket, &mut [0; 256])).unwrap().0;
                continue;
            }
            let dev = &mut device_list[poll_fd_id];
            let events = dev.fetch_events()?;
            for event in events {
                if event.event_type() != evdev::EventType::KEY {
                    continue;
                }
                let event_val = event.value();
                let code = KeyCode::new(event.code());
                let is_key_modi = modifiers::modifier_from_keycode(code);
                let event_state = if event_val == 0 {
                    // On release, remove the modifier.
                    cur_modifiers_per_dev[poll_fd_id] &= !is_key_modi;
                    KeyState::Released
                } else if event_val == 1 {
                    // On press, add the modifier.
                    cur_modifiers_per_dev[poll_fd_id] |= is_key_modi;
                    KeyState::Pressed
                } else {
                    KeyState::Repeated
                };
                let cur_modi = cur_modifiers_per_dev[poll_fd_id];
                let bind = Keybind::new(code, event_state, cur_modi);
                if bindings.contains(&bind) {
                    postcard::to_io(&bind, &input_socket).expect("socket should send successfully");
                    continue;
                }
                // libinput ignores keyrepeats, so this app does too.
                // sending it does nothing.
                // Repeats are handled by the compositor directly.
                if event_state == KeyState::Repeated {
                    continue;
                }
                let key_action = KeyAction::new(code, event_state);
                message::send_msg(
                    &uinput_channel,
                    message::MsgToUInput::SendKeyActions(vec![key_action]),
                );
            }
        }
    }
    Ok(())
}
