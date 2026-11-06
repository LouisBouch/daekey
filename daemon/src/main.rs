use std::{
    error::Error,
    fmt::{Display, write},
    process::ExitCode,
};

mod client_gateway;
pub mod compositor_interface;
mod core;
mod device_interface;
mod fd_transfer;
pub mod ipc;
pub mod logical_device;
pub mod watcher;

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
    // Obtained by running cargo run --bin daemon -- value
    let res = run();
    if let Err(e) = res {
        print_err(&*e);
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
            // Should never return. If it does, it means an error occured.
            return Err(Box::new(core::launch_daemon()) as Box<dyn Error>);
        }
        Some(arg) => {
            if arg == helper_subcommand {
                fd_transfer::run_fd_transferer().map_err(|e| Box::new(e) as Box<dyn Error>)
            } else {
                eprintln!(
                    "Invalid flag/subcommand. Either run daemon without arguments or with the `helper` subcommand to obtain a socket connection from stdout."
                );
                return Err(Box::new(RunError::Arguments) as Box<dyn Error>);
            }
        }
    }
}
#[derive(Debug)]
pub struct ErrorContext<'a> {
    context: String,
    source: &'a (dyn Error + 'static),
}
impl<'a> ErrorContext<'a> {
    pub fn new(context: String, source: &'a (dyn Error + 'static)) -> Self {
        Self { context, source }
    }
}
impl<'a> Display for ErrorContext<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.context)
    }
}
impl<'a> Error for ErrorContext<'a> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source)
    }
}
/// Handle the formatting and printing of an error.
fn print_err(err: &dyn Error) {
    eprintln!("error: {err}");
    let mut source = err.source();
    while let Some(s) = source {
        eprintln!("  caused by: {s}");
        source = s.source();
    }
}

// TODO: For all errors, use named struct instead of tuple?
// TODO: Go around all eprintln and replace them with custom errors when necessary. also check formatting.
