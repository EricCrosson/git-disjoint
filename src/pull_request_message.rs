use std::fmt::Display;

use git2::Commit;

pub(crate) const IGNORE_MARKER: &str = "# ------------------------ >8 ------------------------";

const PULL_REQUEST_INSTRUCTIONS: &str = r#"
# Do not modify or remove the line above.
# Everything below it will be ignored.

Write a message for this pull request. The first block
of text is the title and the rest is the description.

Changes:
"#;

#[derive(Clone, Debug)]
pub(crate) struct PullRequestMessageTemplate<'repo> {
    commits: Vec<&'repo Commit<'repo>>,
}

impl<'repo> FromIterator<&'repo Commit<'repo>> for PullRequestMessageTemplate<'repo> {
    fn from_iter<T: IntoIterator<Item = &'repo Commit<'repo>>>(iter: T) -> Self {
        Self {
            commits: iter.into_iter().collect(),
        }
    }
}

impl<'repo> Display for PullRequestMessageTemplate<'repo> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\n{}\n{}", IGNORE_MARKER, PULL_REQUEST_INSTRUCTIONS)?;
        for commit in &self.commits {
            writeln!(
                f,
                "{:.7} ({:?})",
                &commit.id(),
                commit.author().name().unwrap_or("unknown"),
            )?;
            for line in commit.message().unwrap_or_default().lines() {
                writeln!(f, "    {line}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
