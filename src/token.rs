use std::fmt::Display;
use std::process::{Command, ExitStatus};

#[derive(Debug)]
pub struct ResolveTokenError {
    kind: ResolveTokenErrorKind,
}

impl Display for ResolveTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unable to resolve GitHub token\n\n\
             Provide a token using one of these methods (in order of precedence):\n  \
             1. --github-token <TOKEN>\n  \
             2. GITHUB_TOKEN environment variable\n  \
             3. Install and authenticate the GitHub CLI: gh auth login"
        )
    }
}

impl std::error::Error for ResolveTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ResolveTokenErrorKind::Io(err) => Some(err),
            ResolveTokenErrorKind::NonZeroExit { .. } | ResolveTokenErrorKind::EmptyToken => None,
            ResolveTokenErrorKind::InvalidUtf8(err) => Some(err),
        }
    }
}

#[derive(Debug)]
#[allow(dead_code)]
enum ResolveTokenErrorKind {
    Io(std::io::Error),
    NonZeroExit { status: ExitStatus, stderr: String },
    EmptyToken,
    InvalidUtf8(std::string::FromUtf8Error),
}

/// Resolve a GitHub token by invoking `gh auth token --hostname github.com`.
pub fn resolve_token_from_gh_cli() -> Result<String, ResolveTokenError> {
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .output()
        .map_err(|err| ResolveTokenError {
            kind: ResolveTokenErrorKind::Io(err),
        })?;

    parse_gh_output(output.status, &output.stdout, &output.stderr)
}

fn parse_gh_output(
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<String, ResolveTokenError> {
    if !status.success() {
        let stderr = String::from_utf8_lossy(stderr).into_owned();
        return Err(ResolveTokenError {
            kind: ResolveTokenErrorKind::NonZeroExit { status, stderr },
        });
    }

    let token = String::from_utf8(stdout.to_vec()).map_err(|err| ResolveTokenError {
        kind: ResolveTokenErrorKind::InvalidUtf8(err),
    })?;
    let token = token.trim().to_string();

    if token.is_empty() {
        return Err(ResolveTokenError {
            kind: ResolveTokenErrorKind::EmptyToken,
        });
    }

    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn exit_status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(unix)]
    #[test]
    fn successful_output_returns_trimmed_token() {
        let result = parse_gh_output(exit_status(0), b"gho_abc123\n", b"");
        assert_eq!(result.unwrap(), "gho_abc123");
    }

    #[cfg(unix)]
    #[test]
    fn trailing_whitespace_is_trimmed() {
        let result = parse_gh_output(exit_status(0), b"  gho_abc123  \n", b"");
        assert_eq!(result.unwrap(), "gho_abc123");
    }

    #[cfg(unix)]
    #[test]
    fn non_zero_exit_returns_error() {
        let result = parse_gh_output(exit_status(1), b"", b"not logged in");
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn empty_stdout_returns_error() {
        let result = parse_gh_output(exit_status(0), b"", b"");
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn whitespace_only_stdout_returns_error() {
        let result = parse_gh_output(exit_status(0), b"  \n  ", b"");
        assert!(result.is_err());
    }

    #[test]
    fn error_display_message() {
        let err = ResolveTokenError {
            kind: ResolveTokenErrorKind::EmptyToken,
        };
        insta::assert_snapshot!(err.to_string());
    }
}
