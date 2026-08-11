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
use evdev::{Device, FetchEventsSynced, KeyCode, RelativeAxisCode};
use libdae::{
    input::{KeyAction, KeyState, Keybind, MouseAction, MouseRelAction},
    message, modifiers,
};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

#[derive(Clone, Copy)]
enum DevType {
    Kbd,
    Mouse,
}

pub fn launch_input_listener(
    // bindings: HashSet<Keybind>,
    input_socket: UnixStream,
    uinput_channel: Sender<message::MsgToUInput>,
) -> std::io::Result<InputShare> {
    let dir = std::path::Path::new("/dev/input/by-path/");
    let files = std::fs::read_dir(dir)?;
    let mut device_paths: Vec<(std::path::PathBuf, DevType)> = Vec::new();
    for file in files {
        let file = file.expect("file should be valid directory entry");
        let name = file
            .file_name()
            .into_string()
            .expect("filename should not contain invalid characters");
        // Fetch keyboards.
        let devtype = if name.contains("event-kbd") {
            Some(DevType::Kbd)
        }
        // Fetch mice.
        else if name.contains("event-mouse") {
            Some(DevType::Mouse)
        } else {
            None
        };
        if let Some(devtype) = devtype {
            device_paths.push((
                std::fs::canonicalize(dir.join(std::fs::read_link(dir.join(name.clone()))?))?,
                devtype,
            ));
        }
    }
    // Ensure path uniqueness.
    {
        let mut set: HashSet<PathBuf> = HashSet::new();
        device_paths.retain(|v| set.insert(v.0.clone()));
    }

    let mut device_list = Vec::new();
    for device_path in device_paths {
        let mut dev = evdev::Device::open(device_path.0.clone())?;
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
        device_list.push((dev, device_path.1));
    }
    let handle = std::thread::spawn(move || {
        input_loop(device_list, uinput_channel, input_socket).expect("input loop should not fail");
    });
    Ok(InputShare { handle })
}
/// Wrapper for the file descriptor that contains its source.
struct WrappedFd<'fd> {
    /// The file descriptor.
    fd: BorrowedFd<'fd>,
    /// Source of the fd.
    source: FdSource,
}
/// Source of a file descriptor.
enum FdSource {
    /// Source of fd is an input device
    ///
    /// # Parameters
    ///
    /// * `usize` - The id of the device in the device list.
    Device(usize),
    /// Source of fd is a socket.
    Socket,
}
fn input_loop(
    mut device_list: Vec<(Device, DevType)>,
    uinput_channel: Sender<message::MsgToUInput>,
    input_socket: UnixStream,
) -> std::io::Result<()> {
    let mut bindings: HashSet<Keybind> = HashSet::new();
    // Listen to devices.
    let mut wrapped_fds: Vec<_> = device_list
        .iter()
        .enumerate()
        .map(|(i, v)| unsafe {
            WrappedFd {
                fd: BorrowedFd::borrow_raw(v.0.as_raw_fd()),
                source: FdSource::Device(i),
            }
        })
        .collect();
    wrapped_fds.push(unsafe {
        WrappedFd {
            fd: BorrowedFd::borrow_raw(input_socket.as_raw_fd()),
            source: FdSource::Socket,
        }
    });
    let mut cur_modifiers_per_dev: Vec<modifiers::Modifiers> =
        vec![modifiers::NONE; device_list.len()];

    let mut key_actions: Vec<KeyAction> = Vec::with_capacity(4);
    let mut mouse_actions: Vec<MouseAction> = Vec::with_capacity(4);
    let mut poll_fds: Vec<PollFd> = wrapped_fds
        .iter()
        .map(|v| nix::poll::PollFd::new(v.fd, PollFlags::POLLIN))
        .collect();
    loop {
        if poll(&mut poll_fds, PollTimeout::NONE).is_err() {
            break;
        }
        // TODO: TEST IT AGAIN LATER.
        // let now = Instant::now();
        // println!("sent: {:?}", now);

        let sf_exp = "status flag should be valid";
        // Find which device triggered.
        for (wrapped_fd, poll_fd) in wrapped_fds.iter().zip(poll_fds.iter()) {
            if !poll_fd.any().expect(sf_exp) {
                continue;
            }
            match wrapped_fd.source {
                FdSource::Device(dev_id) => match &mut device_list[dev_id] {
                    (dev, DevType::Kbd) => handle_kbd_events(
                        dev.fetch_events()?,
                        &mut key_actions,
                        &mut cur_modifiers_per_dev[dev_id],
                        &mut bindings,
                        &uinput_channel,
                        &input_socket,
                    )?,
                    (dev, DevType::Mouse) => handle_mouse_events(
                        dev.fetch_events()?,
                        &mut mouse_actions,
                        &mut cur_modifiers_per_dev[dev_id],
                        &mut bindings,
                        &uinput_channel,
                        &input_socket,
                    )?,
                },
                FdSource::Socket => {
                    let msg: message::MsgToInput =
                        postcard::from_io((&input_socket, &mut [0; 256])).unwrap().0;
                    match msg {
                        message::MsgToInput::ChangeBindings(new_bindings) => {
                            bindings = new_bindings
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn handle_kbd_events(
    events: FetchEventsSynced<'_>,
    key_actions: &mut Vec<KeyAction>,
    cur_modifiers: &mut modifiers::Modifiers,
    bindings: &mut HashSet<Keybind>,
    uinput_channel: &Sender<message::MsgToUInput>,
    input_socket: &UnixStream,
) -> std::io::Result<()> {
    for event in events {
        if event.event_type() == evdev::EventType::SYNCHRONIZATION {
            message::send_msg(
                &uinput_channel,
                message::MsgToUInput::SendKeyActions(key_actions.clone()),
            );
            key_actions.clear();
            continue;
        } else if event.event_type() != evdev::EventType::KEY {
            continue;
        }
        let event_val = event.value();
        let code = KeyCode::new(event.code());
        let is_key_modi = modifiers::modifier_from_keycode(code);
        let event_state = if event_val == 0 {
            // On release, remove the modifier.
            *cur_modifiers &= !is_key_modi;
            KeyState::Released
        } else if event_val == 1 {
            // On press, add the modifier.
            *cur_modifiers |= is_key_modi;
            KeyState::Pressed
        } else {
            KeyState::Repeated
        };
        let bind = Keybind::new(code, event_state, *cur_modifiers);
        if bindings.contains(&bind) {
            postcard::to_io(&bind, input_socket).expect("socket should send successfully");
            continue;
        }
        // libinput ignores keyrepeats, so this app does too.
        // sending it does nothing.
        // Repeats are handled by the compositor directly.
        if event_state == KeyState::Repeated {
            continue;
        }
        key_actions.push(KeyAction::new(code, event_state));
    }
    Ok(())
}

fn handle_mouse_events(
    events: FetchEventsSynced<'_>,
    mouse_actions: &mut Vec<MouseAction>,
    cur_modifiers: &mut modifiers::Modifiers,
    bindings: &mut HashSet<Keybind>,
    uinput_channel: &Sender<message::MsgToUInput>,
    input_socket: &UnixStream,
) -> std::io::Result<()> {
    for event in events {
        if event.event_type() == evdev::EventType::SYNCHRONIZATION {
            message::send_msg(
                &uinput_channel,
                message::MsgToUInput::SendMouseActions(mouse_actions.clone()),
            );
            mouse_actions.clear();
            continue;
        } else if event.event_type() == evdev::EventType::KEY {
            let event_val = event.value();
            let code = KeyCode::new(event.code());
            let is_key_modi = modifiers::modifier_from_keycode(code);
            let event_state = if event_val == 0 {
                // On release, remove the modifier.
                *cur_modifiers &= !is_key_modi;
                KeyState::Released
            } else if event_val == 1 {
                // On press, add the modifier.
                *cur_modifiers |= is_key_modi;
                KeyState::Pressed
            } else {
                KeyState::Repeated
            };
            let bind = Keybind::new(code, event_state, *cur_modifiers);
            if bindings.contains(&bind) {
                postcard::to_io(&bind, input_socket).expect("socket should send successfully");
                continue;
            }
            // libinput ignores keyrepeats, so this app does too.
            // sending it does nothing.
            // Repeats are handled by the compositor directly.
            if event_state == KeyState::Repeated {
                continue;
            }
            mouse_actions.push(MouseAction::Key(KeyAction::new(code, event_state)));
        } else if event.event_type() == evdev::EventType::RELATIVE {
            let event_val = event.value();
            let code = RelativeAxisCode(event.code());
            mouse_actions.push(MouseAction::Rel(MouseRelAction::new(code, event_val)));
        }
    }
    Ok(())
}
