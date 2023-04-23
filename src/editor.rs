use std::{
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::{self, Write},
    path::Path,
    process::Command,
};

use git2::Commit;

use crate::{
    pull_request_message::PullRequestMessageTemplate,
    pull_request_metadata::{self, PullRequestMetadata},
};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct GetPullRequestMetadataError {
    kind: GetPullRequestMetadataErrorKind,
}

impl Display for GetPullRequestMetadataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            GetPullRequestMetadataErrorKind::AmbiguousEditor => write!(
                f,
                "unknown editor -- user should set VISUAL or EDITOR environment variable"
            ),
            GetPullRequestMetadataErrorKind::CreateFile(_) => {
                write!(f, "unable to create .git/PULLREQ_MSG file")
            }
            GetPullRequestMetadataErrorKind::BufferWrite(_) => {
                write!(f, "error writing to .git/PULLREQ_MSG file")
            }
            GetPullRequestMetadataErrorKind::EmptyPullRequest(_) => {
                write!(f, "user gave abort signal")
            }
            GetPullRequestMetadataErrorKind::Editor(_) => write!(f, "error invoking editor"),
            GetPullRequestMetadataErrorKind::ReadFile(_) => {
                write!(f, "error reading .git/PULLREQ_MSG file")
            }
        }
    }
}

impl Error for GetPullRequestMetadataError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            GetPullRequestMetadataErrorKind::AmbiguousEditor => None,
            GetPullRequestMetadataErrorKind::CreateFile(err) => Some(err),
            GetPullRequestMetadataErrorKind::BufferWrite(err) => Some(err),
            GetPullRequestMetadataErrorKind::EmptyPullRequest(err) => Some(err),
            GetPullRequestMetadataErrorKind::Editor(err) => Some(err),
            GetPullRequestMetadataErrorKind::ReadFile(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum GetPullRequestMetadataErrorKind {
    #[non_exhaustive]
    AmbiguousEditor,
    #[non_exhaustive]
    CreateFile(io::Error),
    #[non_exhaustive]
    BufferWrite(io::Error),
    #[non_exhaustive]
    Editor(io::Error),
    #[non_exhaustive]
    ReadFile(io::Error),
    #[non_exhaustive]
    EmptyPullRequest(pull_request_metadata::FromStrError),
}

impl From<GetPullRequestMetadataErrorKind> for GetPullRequestMetadataError {
    fn from(kind: GetPullRequestMetadataErrorKind) -> Self {
        Self { kind }
    }
}

pub(crate) fn interactive_get_pr_metadata<'repo>(
    root: &Path,
    commits: impl IntoIterator<Item = impl Into<&'repo Commit<'repo>>>,
) -> Result<PullRequestMetadata, GetPullRequestMetadataError> {
    let editor = get_editor().ok_or(GetPullRequestMetadataErrorKind::AmbiguousEditor)?;

    let file_path = root.join(".git").join("PULLREQ_MSG");
    let mut buffer =
        File::create(&file_path).map_err(GetPullRequestMetadataErrorKind::CreateFile)?;

    writeln!(
        buffer,
        "{}",
        commits
            .into_iter()
            .map(Into::into)
            .collect::<PullRequestMessageTemplate>()
    )
    .map_err(GetPullRequestMetadataErrorKind::BufferWrite)?;

    Command::new(editor)
        .arg(&file_path)
        .status()
        .map_err(GetPullRequestMetadataErrorKind::Editor)?;

    let file_content =
        fs::read_to_string(file_path).map_err(GetPullRequestMetadataErrorKind::ReadFile)?;

    Ok(file_content
        .parse()
        .map_err(GetPullRequestMetadataErrorKind::EmptyPullRequest)?)
}

fn get_editor() -> Option<String> {
    use std::env::var;
    var("VISUAL").or_else(|_| var("EDITOR")).ok()
}
