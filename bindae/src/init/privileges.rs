//! Handles initial privilege dropping at the launch of the program.

static INPUT_GROUP: &str = "input";
static UINPUT_GROUP: &str = "uinput";

/// Error when initializing privleges.
pub enum Error {
    /// The app has not been run as root.
    NotRunningAsRoot,
    NoDefinedUser,
}

/// Ensure the user is currently running as root.
///
/// # Return
///
/// The user that called the program using sudo.
fn ensure_root() {
    let is_root = nix::unistd::Uid::effective().is_root();
    let user = 
         std::env::var("SUDO_USER").expect("user id should be fetchable");
}

/// Create necessary groups.
fn create_groups() {
}

/// Update groups of the UInput file to allow the new group to access it.
fn set_groups() {
}

/// Set privileges to what the application needs to function with UInput and Input.
fn set_privs() {
}

/// Initialize correct permissions for the daemon to run.
pub fn establish_privileges() {

}
