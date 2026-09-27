use crate::{
    device_interface::{input::Input, uinput::UInput},
    ipc::connector,
    router::Router,
};

pub mod ipc;
pub mod compositor_interface;
mod device_interface;
mod router;
mod connection_helper;
mod core;

fn main() {
    // TODO: Use clap to detect if subcommand requiring scm helper is deployed.
    // let args: Vec<String> = std::env::args().collect();
    // args.iter().any(|v| v ==)
    core::launch_daemon();
}
