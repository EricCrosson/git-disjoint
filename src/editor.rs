use std::{fs::File, io::Write, path::Path, process::Command, str::FromStr};

use anyhow::{anyhow, ensure};
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

impl FromStr for PullRequestMetadata {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iterator = s.lines();
        let title = iterator.next().unwrap_or_default().trim().to_owned();
        let body = iterator.collect::<Vec<_>>().join("\n").trim().to_owned();

        Ok(Self { title, body })
    }
}

fn get_editor() -> Option<String> {
    use std::env::var;

    if let Ok(editor) = var("VISUAL").or_else(|_| var("EDITOR")) {
        return Some(editor);
    }
    None
}

fn truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

pub(crate) fn interactive_get_pr_metadata<'repo>(
    root: &Path,
    commits: Vec<&Commit<'repo>>,
) -> Result<PullRequestMetadata, anyhow::Error> {
    let editor = get_editor()
        .ok_or_else(|| anyhow!("User should set VISUAL or EDITOR environment variable"))?;

    let file_path = root.join(".git").join("PULLREQ_MSG");

    {
        let mut buffer = File::create(&file_path)?;

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
    }

    Command::new(editor).arg(&file_path).status()?;

    let file_content = std::fs::read_to_string(file_path)?;

    let file_content = file_content
        .lines()
        .take_while(|line| line != &IGNORE_MARKER)
        .collect::<Vec<_>>()
        .join("\n");

    // If the file is empty, assume the user wants to abort
    ensure!(
        !file_content.is_empty(),
        "Pull request metadata is empty, aborting"
    );

    PullRequestMetadata::from_str(&file_content)
}
