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
            IssueGroup::Issue(issue) => write!(f, "{issue}"),
            IssueGroup::Commit(commit) => write!(f, "{commit}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{issue_group::GitCommitSummary, Issue, IssueGroup};

    #[test]
    fn display_human_readable_issue() {
        let issue = Issue::Jira("COOL-123".to_string());
        let issue_group = IssueGroup::Issue(issue);
        assert_eq!("Jira COOL-123", format!("{issue_group}"));
    }

    #[test]
    fn display_human_readable_commit() {
        let summary = GitCommitSummary(String::from("this is a cool summary"));
        let issue_group = IssueGroup::Commit(summary);
        assert_eq!("this is a cool summary", format!("{issue_group}"));
    }
}
