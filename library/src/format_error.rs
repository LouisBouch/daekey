//! Allows easy formatting and printing of errors to stderr.

use std::{error::Error, fmt::Display};

#[derive(Debug)]
/// Extra textual context for an error.
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
pub fn print_err(err: &dyn Error) {
    eprintln!("error: {err}");
    let mut source = err.source();
    while let Some(s) = source {
        eprintln!("  cause: {s}");
        source = s.source();
    }
}
