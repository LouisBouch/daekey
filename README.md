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

- [ ] Custom ThreadPool struct that can hold a generic type.
- [ ] An executor struct that holds a threadpool and a closure. Uses the threadpool to run the closure on multiple threads at the same time.
- [ ] Split project into an additonal common library.
  * daemon imports common library
  * client_library imports common library and re-exports some necessary items for the client.
  * Client only imports client_library
- [ ] An executor struct that holds a threadpool and a closure. Uses the threadpool to run the closure on multiple threads at the same time.
- [ ] Make the daemon listen for compositor changes (like the screen layout) and
      push them to the connected users afterwards. Also give the scripts the
      compositor state at the start.

Once this is done, additional features will include:

- [ ] A macro maker that fetches initiale absolute cursor position on a key
      press and then record everything that is done.
- [ ] A API call that allows you to get the pixel color under the cursor.
- [ ] A API call that allows you to find the position of a pixel with a certain
      color.
