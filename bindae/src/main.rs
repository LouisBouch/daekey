use std::{error::Error, fmt::Display, process::ExitCode};

pub mod compositor_interface;
mod fd_transfer;
mod core;
mod device_interface;
pub mod ipc;
mod router;

/// Errors that can occur when launching the daemon.
#[derive(Debug)]
pub enum RunError {
    /// Arguments given to binary are invalid.
    Arguments,
}
impl Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Arguments => {
                write!(
                    f,
                    "arguments given to the binary are invalid, must either be run without any arguments or with the `helper` subcommand"
                )
            }
        }
    }
}
impl Error for RunError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            RunError::Arguments => None,
        }
    }
}
fn main() -> ExitCode {
    // Obtained by running cargo run --bin bindae -- value
    let res = run();
    if let Err(e) = res {
        print_err(e);
        return ExitCode::FAILURE;
    };
    ExitCode::SUCCESS
}
/// Run the daemon.
fn run() -> Result<(), Box<dyn Error + 'static>> {
    let arg = std::env::args().nth(1);
    let helper_subcommand = "helper";
    match arg {
        None => {
            // TODO: Add result return to launch_daemon function.
            core::launch_daemon();
            Ok(())
        }
        Some(arg) => {
            if arg == helper_subcommand {
                fd_transfer::run_fd_transferer()
                    .map_err(|e| Box::new(e) as Box<dyn Error>)
            } else {
                eprintln!(
                    "Invalid flag/subcommand. Either run daemon without arguments or with the `helper` subcommand to obtain a socket connection from stdout."
                );
                return Err(Box::new(RunError::Arguments) as Box<dyn Error>);
            }
        }
    }
}
/// Handle the formatting and printing of an error.
fn print_err(err: Box<dyn Error + 'static>) {
    eprintln!("error: {err}");
    let mut source = err.source();
    while let Some(s) = source {
        eprintln!(" caused by: {err}");
        source = s.source();
    }
}
