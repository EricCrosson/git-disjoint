use std::{
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::{self, Write},
    path::Path,
    process::Command,
};

use git2::Commit;

const PULL_REQUEST_HEADER: &str = r#"
# ------------------------ >8 ------------------------
# Do not modify or remove the line above.
# Everything below it will be ignored.

Write a message for this pull request. The first block
of text is the title and the rest is the description.

Changes:
"#;

const IGNORE_MARKER: &str = "# ------------------------ >8 ------------------------";

#[derive(Debug)]
pub(crate) struct PullRequestMetadata {
    pub title: String,
    pub body: String,
}

impl From<String> for PullRequestMetadata {
    fn from(s: String) -> Self {
        let mut iterator = s.lines();
        let title = iterator.next().unwrap_or_default().trim().to_owned();
        let body = iterator.collect::<Vec<_>>().join("\n").trim().to_owned();

        Self { title, body }
    }
}

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
            GetPullRequestMetadataErrorKind::EmptyPullRequestBody => {
                write!(f, "pull request metadata is empty -- aborting")
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
            GetPullRequestMetadataErrorKind::EmptyPullRequestBody => None,
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
    EmptyPullRequestBody,
}

impl From<GetPullRequestMetadataErrorKind> for GetPullRequestMetadataError {
    fn from(kind: GetPullRequestMetadataErrorKind) -> Self {
        Self { kind }
    }
}

pub(crate) fn interactive_get_pr_metadata<'repo>(
    root: &Path,
    commits: Vec<&Commit<'repo>>,
) -> Result<PullRequestMetadata, GetPullRequestMetadataError> {
    let editor = get_editor().ok_or(GetPullRequestMetadataErrorKind::AmbiguousEditor)?;

    let file_path = root.join(".git").join("PULLREQ_MSG");
    let mut buffer =
        File::create(&file_path).map_err(GetPullRequestMetadataErrorKind::CreateFile)?;

    (|| {
        // Write template to file
        writeln!(buffer, "{PULL_REQUEST_HEADER}")?;
        for commit in commits {
            writeln!(
                buffer,
                "{} ({:?})",
                truncate(&commit.id().to_string(), 7),
                commit.author().name().unwrap_or_default(),
            )?;
            for line in commit.message().unwrap_or_default().lines() {
                writeln!(buffer, "    {line}")?;
            }
            writeln!(buffer)?;
        }
        buffer.flush()?;
        Ok(())
    })()
    .map_err(GetPullRequestMetadataErrorKind::BufferWrite)?;

    Command::new(editor)
        .arg(&file_path)
        .status()
        .map_err(GetPullRequestMetadataErrorKind::Editor)?;

    let file_content =
        fs::read_to_string(file_path).map_err(GetPullRequestMetadataErrorKind::ReadFile)?;

    let file_content = file_content
        .lines()
        .take_while(|line| line != &IGNORE_MARKER)
        .collect::<Vec<_>>()
        .join("\n");

    // If the file is empty, assume the user wants to abort
    if file_content.is_empty() {
        return Err(GetPullRequestMetadataErrorKind::EmptyPullRequestBody)?;
    }

    Ok(file_content.into())
}

fn get_editor() -> Option<String> {
    use std::env::var;
    var("VISUAL").or_else(|_| var("EDITOR")).ok()
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
