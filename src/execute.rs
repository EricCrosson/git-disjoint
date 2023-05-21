use std::error::Error;
use std::fmt::Display;
use std::fs::OpenOptions;
use std::io::{self, prelude::*};
use std::process::{Command, ExitStatusError, Stdio};

use crate::log_file::LogFile;

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct ExecuteError {
    kind: ExecuteErrorKind,
}

impl Display for ExecuteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ExecuteErrorKind::Write(_) => write!(f, "unable to write to stream"),
            ExecuteErrorKind::Exec(_, command) => {
                write!(f, "unable to execute command: {}", command.join(" "))
            }
            ExecuteErrorKind::Child(_, command) => write!(
                f,
                "child process exited with non-zero code: {}",
                command.join(" ")
            ),
        }
    }
}

impl Error for ExecuteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ExecuteErrorKind::Write(err) => Some(err),
            ExecuteErrorKind::Exec(err, _) => Some(err),
            ExecuteErrorKind::Child(err, _) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ExecuteErrorKind {
    #[non_exhaustive]
    Write(io::Error),
    /// Error while executing the child process
    #[non_exhaustive]
    Exec(io::Error, Vec<String>),
    /// The child process exited with non-zero exit code
    #[non_exhaustive]
    Child(ExitStatusError, Vec<String>),
}

pub(crate) fn execute(command: &[&str], log_file: &LogFile) -> Result<(), ExecuteError> {
    (|| -> Result<(), ExecuteErrorKind> {
        let mut runner = Command::new(command[0]);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect(&format!(
                "should be able to append to log file {:?}",
                log_file
            ));

        writeln!(file, "$ {:?}", command.join(" ")).map_err(ExecuteErrorKind::Write)?;

        // DISCUSS: how to pipe stdout to the same file?
        // Do we need the duct crate?
        // https://stackoverflow.com/a/41025699
        // It's not immediately obvious to me how we pass `command`
        // to a duct `cmd`, but I bet there's a way to separate
        // the head and the tail from our slice.
        runner.stdout(Stdio::null());
        runner.stderr(file);

        for argument in command.iter().skip(1) {
            runner.arg(argument);
        }

        // Try to run the command
        let status = runner.status().map_err(|err| {
            ExecuteErrorKind::Exec(
                err,
                command
                    .iter()
                    .map(ToOwned::to_owned)
                    .map(ToOwned::to_owned)
                    .collect(),
            )
        })?;

        // Return an Err if the exit status is non-zero
        if let Err(error) = status.exit_ok() {
            return Err(ExecuteErrorKind::Child(
                error,
                command
                    .iter()
                    .map(ToOwned::to_owned)
                    .map(ToOwned::to_owned)
                    .collect(),
            ));
        }
        Ok(())
    })()
    .map_err(|kind| ExecuteError { kind })
}
