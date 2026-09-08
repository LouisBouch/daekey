An AHK-like app for Wayland-wlroots setups.

# What does it do?

It allows arbitrary Rust closures enriched with an API to run on defined keybinds, a
bit like AHK does, but with rust closures instead of a scripting language.

# How does it work?

It starts a privileged daemon that listens for device inputs and that can send
keys to the compositor. By itself, the daemon does nothing, but user made
scrips created with the library implemented in this project can interface with it.

When the daemon sees that a pressed key has a binding, it sends it to the user's
script where the defined closure will run. This closure can run any Rust code as
well as request the daemon to send keys/mouse actions to the compositor (more
features to come).

# WIP

Currently, only a single instance can run at a time. Time fix this, the
following code changes will be made:

- [ ] Unbind the daemon from the core process. Currently, they must run
  together, so allow them to just run independently. Run the daemon with
  input,uinput and daekey groups.
- [ ] Each connection to the daemon should be done through a single socket
  instead of the many there currently are. This socket will have daekey:dakey
  permissions. To access this socket from the connecting process, it can simply
  re-exec itself with sudo privileges and a speciall flag that will fetch the
  socket and return it with SCM_RIGHTS.
- [ ] The daemon's input manager will listen to input devices AND to a single
  byte fd that will notify it when it received a message from a user's script.
  So the user sends a message through the socket, the message manager sends
  message through crossbeam to the input manager AND activates the fd byte so
  that the input manager can act on it, as it will only poll fds.
- [ ] Make the daemon listen for compositor changes (like the screen layout) and
  push them to the connected users afterwards. Also give the scripts the
  compositor state at the start.

Once this is done, additional features will include:

- [ ] A macro maker that fetches initiale absolute cursor position on a key
  press and then record everything that is done.
- [ ] A API call that allows you to get the pixel color under the cursor.
- [ ] A API call that allows you to find the position of a pixel with a certain
  color.
