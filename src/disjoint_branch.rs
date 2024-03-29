use std::{collections::HashSet, error::Error, fmt::Display};

use git2::Commit;
use indexmap::IndexMap;

use crate::{branch_name::BranchName, issue_group::IssueGroup, issue_group_map::IssueGroupMap};

#[derive(Debug)]
pub(crate) struct DisjointBranch<'repo> {
    // REFACTOR: make this private
    pub branch_name: BranchName,
    // REFACTOR: make this private
    pub commits: Vec<Commit<'repo>>,
}

#[derive(Debug)]
pub(crate) struct DisjointBranchMap<'repo>(IndexMap<IssueGroup, DisjointBranch<'repo>>);

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct FromIssueGroupMapError {
    kind: FromIssueGroupMapErrorKind,
}

impl Display for FromIssueGroupMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FromIssueGroupMapErrorKind::InvalidUtf8(commit) => {
                write!(f, "commit summary contains invalid UTF-8: {}", commit)
            }
        }
    }
}

impl Error for FromIssueGroupMapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FromIssueGroupMapErrorKind::InvalidUtf8(_) => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum FromIssueGroupMapErrorKind {
    #[non_exhaustive]
    InvalidUtf8(String),
}

impl From<FromIssueGroupMapErrorKind> for FromIssueGroupMapError {
    fn from(kind: FromIssueGroupMapErrorKind) -> Self {
        Self { kind }
    }
}

impl<'repo> FromIterator<(IssueGroup, DisjointBranch<'repo>)> for DisjointBranchMap<'repo> {
    fn from_iter<T: IntoIterator<Item = (IssueGroup, DisjointBranch<'repo>)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<'repo> IntoIterator for DisjointBranchMap<'repo> {
    type Item = (IssueGroup, DisjointBranch<'repo>);

    type IntoIter = indexmap::map::IntoIter<IssueGroup, DisjointBranch<'repo>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'repo> TryFrom<IssueGroupMap<'repo>> for DisjointBranchMap<'repo> {
    type Error = FromIssueGroupMapError;

    /// Plan out branch names to avoid collisions.
    ///
    /// This function does not take into account existing branch names in the local
    /// or remote repository. It only looks at branch names that git-disjoint is
    /// going to generate to make sure one invocation of git-disjoint won't try to
    /// create a branch with the same name twice.
    fn try_from(commits_by_issue_group: IssueGroupMap<'repo>) -> Result<Self, Self::Error> {
        let mut suffix: u32 = 0;
        let mut seen_branch_names = HashSet::new();
        commits_by_issue_group
            .into_iter()
            .map(|(issue_group, commits)| {
                // Grab the first summary to convert into a branch name.
                // We only choose the first summary because we know each Vec is
                // non-empty and the first element is convenient.
                let summary = {
                    let commit = &commits[0];
                    commit.summary().ok_or_else(|| {
                        FromIssueGroupMapErrorKind::InvalidUtf8(commit.id().to_string())
                    })?
                };
                let generated_branch_name = BranchName::from_issue_group(&issue_group, summary);
                let mut proposed_branch_name = generated_branch_name.clone();

                while seen_branch_names.contains(&proposed_branch_name) {
                    suffix += 1;
                    // OPTIMIZE: no need to call sanitize_git_ref here again
                    proposed_branch_name = format!("{generated_branch_name}_{suffix}").into();
                }

                seen_branch_names.insert(proposed_branch_name.clone());

                Ok((
                    issue_group,
                    DisjointBranch {
                        branch_name: proposed_branch_name,
                        commits,
                    },
                ))
            })
            .collect()
    }
}
