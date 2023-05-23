use std::fs;

use crate::log_file::LogFile;

/// A vendored error type providing the Debug format from `anyhow::Error`.
///
/// This error type is meant to be used in one place in your binary: the error
/// type of the `Result` your `main` function returns. This converts any error
/// your `main` function produces into a `little_anyhow::Error`, which provides
/// human-readable error messages for your users.
///
/// Error messages look like:
///
/// ```ignore
/// Error: error reading `Blocks.txt`
///
/// Caused by:
///     0: invalid Blocks.txt data on line 223
///     1: one end of range is not a valid hexidecimal integer
///     2: invalid digit found in string
/// ```
///
/// For more information, see:
///
/// - [Modular Errors in Rust]
/// - [little-anyhow]
///
/// [modular errors in rust]: https://sabrinajewson.org/blog/errors#guidelines-for-good-errors
/// [little-anyhow]: https://github.com/EricCrosson/little-anyhow
///
/// # Examples
///
/// ```should_panic
/// use std::io::{self, Write};
///
/// // Return `Result<(), little_anyhow::Error>` from `main` for
/// // human-readable errors from your binary
/// fn main() -> Result<(), little_anyhow::Error> {
///     writeln!(
///         io::stdout(),
///         "You can create a little_anyhow::Error from any type implementing `std::error::Error`"
///     )?;
///
///     let simulated_error = std::fmt::Error;  // an easy-to-create error type
///     Err(simulated_error)?
/// }
/// ```
pub(crate) struct Error {
    err: Box<dyn std::error::Error>,
    log_file: Option<LogFile>,
}

impl Error {
    pub fn new(err: crate::error::Error, log_file: LogFile) -> Self {
        Self {
            err: Box::new(err),
            log_file: Some(log_file),
        }
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.err)?;

        if let Some(source) = self.err.source() {
            write!(f, "\n\nCaused by:")?;
            let mut n: u32 = 0;
            let mut error = Some(source);
            while let Some(current_error) = error {
                write!(f, "\n    {}: {}", n, current_error)?;
                n += 1;
                error = current_error.source();
            }
        }

        if let Some(log_file) = &self.log_file {
            if let Ok(log_contents) = fs::read_to_string(&log_file.0) {
                writeln!(f, "\n\nLog contents:")?;
                writeln!(f, "{}", log_contents)?;
            } else {
                writeln!(f, "\n\nFailed to read log file: {:?}", log_file)?;
            }
            writeln!(f, "\nLog file: {:?}", log_file.0)?;
        } else {
            writeln!(f, "\n\nNo log file available.")?;
        }

        Ok(())
    }
}

impl<E> From<E> for Error
where
    E: std::error::Error + 'static,
{
    fn from(error: E) -> Self {
        Self {
            err: Box::new(error),
            log_file: None,
        }
    }
}
