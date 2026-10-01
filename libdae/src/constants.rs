//! Define constants for usage crate wide.

// TODO: Make the following two paths editable by a user before launchingthe daemon, and synch them
// with the install.sh script.

pub const DAEMON_SOCKET_PATH: &str = "/run/daekey/daekey.socket";
pub const DAEMON_BIN_PATH: &str = "/usr/local/sbin/daekey";
