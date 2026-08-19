//! Handles input devices..
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fmt::Display,
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    path::PathBuf,
    thread::JoinHandle,
    time::{Duration, UNIX_EPOCH},
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
    app::InputPrivSocket,
    input::{KeyAction, KeyState, Keybind, MouseAction, MouseRelAction},
    message, modifiers,
};
use nix::{
    libc::clock_gettime, poll::{PollFd, PollFlags, PollTimeout, poll}, sys::{time::TimeSpec, timerfd}
};

#[derive(Clone, Copy)]
enum DevType {
    Kbd,
    Mouse,
}
/// Holds information relative to a [`Device`].
struct DeviceInfo {
    /// The [`Device`] itself.
    device: Device,
    /// The type of the [`Device`].
    d_type: DevType,
    /// Modifiers (like shift, ctrl) that the [`Device`] is currently holding down.
    d_modifiers: modifiers::Modifiers,
}

/// Get the device paths for both mouse and keyboard.
fn get_device_paths() -> std::io::Result<Vec<(std::path::PathBuf, DevType)>> {
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
            device_paths.push((std::fs::canonicalize(dir.join(name.clone()))?, devtype));
        }
    }
    // Ensure path uniqueness.
    {
        let mut set: HashSet<PathBuf> = HashSet::new();
        device_paths.retain(|v| set.insert(v.0.clone()));
    }
    Ok(device_paths)
}

/// Fetch currently active devices.
///
/// # Parameters
///
/// * `force_clean_state` - Forces the user to release all keys before fetching devices.
///
/// # Return
///
/// List of [`DeviceInfo`] that describes what [`Device`]s are currently active.
fn fetch_devices(force_clean_state: bool) -> std::io::Result<Vec<DeviceInfo>> {
    let device_paths = get_device_paths()?;
    let mut device_list = Vec::new();
    for device_path in device_paths {
        let mut dev = evdev::Device::open(device_path.0.clone())?;
        // Wait for all keys to be released before grabbing the device.
        if force_clean_state {
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
                            "WARNING: If key was released before ungrab happened, the key will be stuck pressed down until pressed and released again.\n
                            (very very very very very highly unlikely. Requires microsecond precision.)"
                        );
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        } else {
            dev.grab()?;
            println!("grabbed: {:?}", std::time::Instant::now());
            println!(
                "dev: {:?}, nb_pressed: {}",
                dev.name(),
                dev.get_key_state()?.iter().len()
            );
            if !dev.get_key_state()?.iter().len() == 0 {
                println!(
                    "WARNING: Keys were pressed when the device was grabbed.\n
                    This can cause subtle bugs depending on when the key state changed and the context.\n
                    Note: This can be ignored if the keys were already pressed on device update."
                )
            }
        }
        let modifiers = modifiers::modifiers_from_key_codes(&dev.get_key_state()?);
        device_list.push(DeviceInfo {
            device: dev,
            d_type: device_path.1,
            d_modifiers: modifiers,
        });
    }
    Ok(device_list)
}

pub fn launch_input_listener(
    input_socket: InputPrivSocket,
    uinput_channel: Sender<message::MsgToUInput>,
    min_mouse_poll_interval: Duration,
) -> std::io::Result<InputShare> {
    let handle = std::thread::spawn(move || {
        input_loop(uinput_channel, input_socket, min_mouse_poll_interval)
            .expect("input loop should not fail");
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
#[derive(Debug)]
enum FdSource {
    /// Source of fd is an input device
    ///
    /// # Parameters
    ///
    /// * `usize` - The id of the device in the device list.
    Device(usize),
    /// Source of fd is a socket.
    Socket,
    /// An fd timer ran out and activated.
    ///
    /// # Parameters
    ///
    /// * `FdTimerReason` - The reason why the timer triggered.
    Timer(FdTimerReason),
}
/// The reason why an fd timer triggered.
#[derive(Debug)]
enum FdTimerReason {
    /// accumulated relative mouse actions need to be sent. Usually done because values accumulated between
    /// minimum poll intervals.
    SendAccRelMouseActions,
}
fn input_loop(
    uinput_channel: Sender<message::MsgToUInput>,
    mut input_socket: InputPrivSocket,
    min_mouse_poll_interval: Duration,
) -> std::io::Result<()> {
    let mut bindings: HashSet<Keybind> = HashSet::new();
    // List of pending actions before a sync event.
    let mut key_actions: Vec<KeyAction> = Vec::with_capacity(4);
    let mut mouse_actions: Vec<MouseAction> = Vec::with_capacity(4);

    // Define the device list and the file descriptors we will populate later.
    let mut all_device_info = Vec::new();
    let mut wrapped_fds = Vec::new();
    let mut poll_fds: Vec<PollFd> = Vec::new();
    let mut update_devices = true;
    let mut first_fetch = true;

    let fd_timer = timerfd::TimerFd::new(
        timerfd::ClockId::CLOCK_REALTIME,
        timerfd::TimerFlags::empty(),
    )?;
    // List of accumualted movements between mouse polls.
    let mut accumulated_relative_mouse_movements: HashMap<evdev::RelativeAxisCode, i32> =
        HashMap::new();
    loop {
        // Populate list of devices and fiel descriptor we listen to.
        if update_devices {
            all_device_info.clear();
            all_device_info = fetch_devices(first_fetch)?;
            wrapped_fds = all_device_info
                .iter()
                .enumerate()
                .map(|(i, v)| unsafe {
                    WrappedFd {
                        fd: BorrowedFd::borrow_raw(v.device.as_raw_fd()),
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
            wrapped_fds.push(WrappedFd {
                fd: fd_timer.as_fd(),
                source: FdSource::Timer(FdTimerReason::SendAccRelMouseActions),
            });
            poll_fds = wrapped_fds
                .iter()
                .map(|v| nix::poll::PollFd::new(v.fd, PollFlags::POLLIN))
                .collect();
            update_devices = false;
            first_fetch = false;
        }

        // Block for message from core or for input from device.
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
            match &wrapped_fd.source {
                FdSource::Device(dev_id) => match &mut all_device_info[*dev_id] {
                    DeviceInfo {
                        device,
                        d_type: DevType::Kbd,
                        d_modifiers,
                    } => handle_kbd_events(
                        device.fetch_events()?,
                        &mut key_actions,
                        d_modifiers,
                        &mut bindings,
                        &uinput_channel,
                        &input_socket,
                    )?,
                    DeviceInfo {
                        device,
                        d_type: DevType::Mouse,
                        d_modifiers,
                    } => handle_mouse_events(
                        device.fetch_events()?,
                        &mut mouse_actions,
                        d_modifiers,
                        &mut bindings,
                        &uinput_channel,
                        &input_socket,
                        min_mouse_poll_interval,
                        &mut accumulated_relative_mouse_movements,
                        &fd_timer,
                    )?,
                },
                FdSource::Socket => {
                    let msg: message::MsgToInput = input_socket.receive().unwrap();
                    match msg {
                        message::MsgToInput::ChangeBindings(new_bindings) => {
                            bindings = new_bindings
                        }
                        message::MsgToInput::PointersChanged
                        | message::MsgToInput::KeyboardChanged => {
                            // If new devices are the same as the old ones, just ignore it.
                            let changed = devices_changed(&all_device_info);
                            if changed.unwrap_or(true) {
                                // When either a mouse or keyboard is update/added/removed,
                                // refetch the devices to makesur they are up to date.
                                update_devices = true;
                            }
                        }
                    }
                }
                FdSource::Timer(fd_timer_reason) => {
                    // Empty the fd so that it stops being readable.
                    fd_timer.wait()?;
                    match fd_timer_reason {
                        FdTimerReason::SendAccRelMouseActions => {
                            let actions = accumulated_relative_mouse_movements
                                .drain()
                                .map(|(rel_axis, value)| {
                                    MouseAction::Rel(MouseRelAction::new(rel_axis, value))
                                })
                                .collect();
                            message::send_msg(
                                &uinput_channel,
                                message::MsgToUInput::SendMouseActions(actions),
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
/// Error when  comparing devices.
enum DevErr {
    DeviceHasNoPhysicalPath,
    DevicePathHasNoFileName,
    IoError(std::io::Error),
}
impl Error for DevErr {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DevErr::DeviceHasNoPhysicalPath => None,
            DevErr::DevicePathHasNoFileName => None,
            DevErr::IoError(error) => Some(error),
        }
    }
}
impl Display for DevErr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error while trying to check devices: {:?}", self)
    }
}
/// Check whether the devices have changed.
fn devices_changed(all_device_info: &[DeviceInfo]) -> Result<bool, DevErr> {
    let mut match_fail = false;
    for new_path in get_device_paths().map_err(DevErr::IoError)? {
        let new_phys_path = std::fs::read_to_string(
            PathBuf::new()
                .join("/sys/class/input/")
                .join(
                    new_path
                        .0
                        .file_name()
                        .ok_or(DevErr::DevicePathHasNoFileName)?,
                )
                .join("device/phys"),
        )
        .map_err(DevErr::IoError)?;
        let mut matched = false;
        for dev_info in all_device_info {
            if new_phys_path.trim()
                == dev_info
                    .device
                    .physical_path()
                    .ok_or(DevErr::DeviceHasNoPhysicalPath)?
                    .trim()
            {
                matched = true;
                continue;
            }
        }
        if !matched {
            match_fail = true;
            break;
        }
    }
    // Devices need to be changed if a match failed.
    Ok(match_fail)
}
fn handle_kbd_events(
    events: FetchEventsSynced<'_>,
    key_actions: &mut Vec<KeyAction>,
    cur_modifiers: &mut modifiers::Modifiers,
    bindings: &mut HashSet<Keybind>,
    uinput_channel: &Sender<message::MsgToUInput>,
    input_socket: &InputPrivSocket,
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
            input_socket
                .send(&bind)
                .expect("socket should send successfully");
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
    input_socket: &InputPrivSocket,
    poll_interval: Duration,
    accumulated_rel_movements: &mut HashMap<evdev::RelativeAxisCode, i32>,
    fd_timer: &timerfd::TimerFd,
) -> std::io::Result<()> {
    for event in events {
        match event.event_type() {
            evdev::EventType::SYNCHRONIZATION => {
                if !poll_interval.is_zero() {
                    // Simulate the next poll for the mouse.
                    if fd_timer.get()?.is_none() {
                        let int_micros = poll_interval.as_micros();
                        let since_epoch = event
                            .timestamp()
                            .duration_since(UNIX_EPOCH)
                            .expect("should be valid duration");
                        let time_until_next_interval = Duration::from_micros(
                            (int_micros - since_epoch.as_micros() % int_micros) as u64,
                        );
                        fd_timer.set(
                            timerfd::Expiration::OneShot(TimeSpec::from_duration(
                                since_epoch + time_until_next_interval,
                            )),
                            timerfd::TimerSetTimeFlags::TFD_TIMER_ABSTIME,
                        )?;
                    }
                    // Remove relative movmenets from the queue and accumulate them.
                    let mut non_rel_mouse_actions: Vec<MouseAction> = Vec::new();
                    for action in mouse_actions.iter() {
                        match action {
                            MouseAction::Rel(mouse_rel_action) => {
                                *accumulated_rel_movements
                                    .entry(mouse_rel_action.axis)
                                    .or_insert(0) += mouse_rel_action.value;
                            }
                            _ => non_rel_mouse_actions.push(action.clone()),
                        }
                    }
                    *mouse_actions = non_rel_mouse_actions;
                }
                message::send_msg(
                    &uinput_channel,
                    message::MsgToUInput::SendMouseActions(std::mem::take(mouse_actions)),
                );
            }
            evdev::EventType::KEY => {
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
                    input_socket
                        .send(&bind)
                        .expect("socket should send successfully");
                    continue;
                }
                // libinput ignores keyrepeats, so this app does too.
                // sending it does nothing.
                // Repeats are handled by the compositor directly.
                if event_state == KeyState::Repeated {
                    continue;
                }
                mouse_actions.push(MouseAction::Key(KeyAction::new(code, event_state)));
            }
            evdev::EventType::RELATIVE => {
                let event_val = event.value();
                let code = RelativeAxisCode(event.code());
                mouse_actions.push(MouseAction::Rel(MouseRelAction::new(code, event_val)));
            }
            _ => (),
        }
    }
    Ok(())
}
