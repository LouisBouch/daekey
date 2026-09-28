pub mod ipc;
pub mod compositor_interface;
mod device_interface;
mod router;
mod connection_helper;
mod core;

fn main() {
    // Obtained by running cargo run --bin bindae -- value
    let arg = std::env::args().nth(1);
    let helper_subcommand = "helper";
    match arg {
        None => core::launch_daemon(),
        Some(arg) => if arg == helper_subcommand {
            connection_helper::launch_connection_helper();
        },
    }
}
