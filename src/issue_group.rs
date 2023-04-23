use std::{error::Error, fmt::Display};

use git2::Commit;

use crate::issue::Issue;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GitCommitSummary(pub String);

impl Display for GitCommitSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct FromCommitError {
    commit: git2::Oid,
    kind: FromCommitErrorKind,
}

impl Display for FromCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FromCommitErrorKind::InvalidUtf8 => {
                write!(f, "summary for commit {:?} is not valid UTF-8", self.commit)
            }
        }
    }
}

impl Error for FromCommitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FromCommitErrorKind::InvalidUtf8 => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum FromCommitErrorKind {
    #[non_exhaustive]
    InvalidUtf8,
}

impl<'a> TryFrom<&Commit<'a>> for GitCommitSummary {
    type Error = FromCommitError;

    fn try_from(commit: &Commit) -> Result<Self, Self::Error> {
        Ok(Self(
            commit
                .summary()
                .ok_or(FromCommitError {
                    commit: commit.id(),
                    kind: FromCommitErrorKind::InvalidUtf8,
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
            IssueGroup::Issue(issue) => write!(f, "{issue}"),
            IssueGroup::Commit(commit) => write!(f, "{commit}"),
        }
    }
}

impl From<GitCommitSummary> for IssueGroup {
    fn from(value: GitCommitSummary) -> Self {
        Self::Commit(value)
    }
}

impl From<Issue> for IssueGroup {
    fn from(value: Issue) -> Self {
        Self::Issue(value)
    }
}

#[cfg(test)]
mod test_display {
    use crate::{issue::Issue, issue_group::GitCommitSummary};

    use super::IssueGroup;

    fn check<I: Into<IssueGroup>>(issue_group: I, displays_as: &str) {
        assert_eq!(displays_as, format!("{}", issue_group.into()));
    }

    #[test]
    fn display_human_readable_issue() {
        check(Issue::Jira("COOL-123".to_string()), "Jira COOL-123");
    }

    #[test]
    fn display_human_readable_commit() {
        check(
            GitCommitSummary(String::from("this is a cool summary")),
            "this is a cool summary",
        );
    }
}
