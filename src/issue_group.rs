use std::fmt::Display;

use anyhow::anyhow;
use git2::Commit;

use crate::issue::Issue;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GitCommitSummary(pub String);

impl Display for GitCommitSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl<'a> TryFrom<&Commit<'a>> for GitCommitSummary {
    type Error = anyhow::Error;

    fn try_from(commit: &Commit) -> Result<Self, Self::Error> {
        Ok(Self(
            commit
                .summary()
                .ok_or_else(|| {
                    anyhow!(
                        "Summary for commit {:?} is not a valid UTF-8 string",
                        commit.id()
                    )
                })?
                .to_owned(),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum IssueGroup {
    Issue(Issue),
    Commit(GitCommitSummary),
}

impl Display for IssueGroup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueGroup::Issue(issue) => write!(f, "{}", issue),
            IssueGroup::Commit(commit) => write!(f, "{}", commit),
        }
    }
}
