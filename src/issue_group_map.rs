use std::{
    collections::HashSet,
    error::Error,
    fmt::Display,
    io::{self, Write},
};

use git2::Commit;
use indexmap::IndexMap;

use crate::{
    cli::{
        CommitGrouping, CommitsToConsider, OverlayCommitsIntoOnePullRequest,
        PromptUserToChooseCommits,
    },
    interact::{prompt_user, IssueGroupWhitelist, SelectIssuesError},
    issue::Issue,
    issue_group::{self, GitCommitSummary, IssueGroup},
};

#[derive(Debug, Default)]
pub(crate) struct IssueGroupMap<'repo>(IndexMap<IssueGroup, Vec<Commit<'repo>>>);

impl<'repo> IntoIterator for IssueGroupMap<'repo> {
    type Item = (IssueGroup, Vec<Commit<'repo>>);

    type IntoIter = indexmap::map::IntoIter<IssueGroup, Vec<Commit<'repo>>>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'repo> FromIterator<(IssueGroup, Vec<Commit<'repo>>)> for IssueGroupMap<'repo> {
    fn from_iter<T: IntoIterator<Item = (IssueGroup, Vec<Commit<'repo>>)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct FromCommitsError {
    kind: FromCommitsErrorKind,
}

impl Display for FromCommitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FromCommitsErrorKind::FromCommit(_) => write!(f, "unable to get commit summary"),
            FromCommitsErrorKind::IO(_) => write!(f, "unable to write to output stream"),
        }
    }
}

impl Error for FromCommitsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FromCommitsErrorKind::FromCommit(err) => Some(err),
            FromCommitsErrorKind::IO(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FromCommitsErrorKind {
    #[non_exhaustive]
    FromCommit(issue_group::FromCommitError),
    #[non_exhaustive]
    IO(io::Error),
}

impl From<issue_group::FromCommitError> for FromCommitsError {
    fn from(err: issue_group::FromCommitError) -> Self {
        Self {
            kind: FromCommitsErrorKind::FromCommit(err),
        }
    }
}

impl From<io::Error> for FromCommitsError {
    fn from(err: io::Error) -> Self {
        Self {
            kind: FromCommitsErrorKind::IO(err),
        }
    }
}

impl<'repo> IssueGroupMap<'repo> {
    fn with_capacity(n: usize) -> Self {
        Self(IndexMap::with_capacity(n))
    }

    fn insert(&mut self, key: IssueGroup, value: Vec<Commit<'repo>>) {
        self.0.insert(key, value);
    }

    pub fn try_from_commits(
        commits: Vec<Commit<'repo>>,
        commits_to_consider: CommitsToConsider,
        commit_grouping: CommitGrouping,
    ) -> Result<Self, FromCommitsError> {
        let mut suffix: u32 = 0;
        let mut seen_issue_groups = HashSet::new();
        let commits_by_issue: IndexMap<IssueGroup, Vec<Commit>> = commits
            .into_iter()
            // Parse issue from commit message
            .map(
                |commit| -> Result<Option<(IssueGroup, Commit)>, FromCommitsError> {
                    let issue = commit.message().and_then(Issue::parse_from_commit_message);
                    // If:
                    // - we're grouping commits by issue, and
                    // - this commit includes an issue,
                    // then add this commit to that issue's group.
                    if commit_grouping == CommitGrouping::ByIssue {
                        if let Some(issue) = issue {
                            return Ok(Some((issue.into(), commit)));
                        }
                    }

                    // If:
                    // - we're treating every commit separately, or
                    // - we're considering all commits (even commits without an issue),
                    // add this commit to a unique issue-group.
                    if commit_grouping == CommitGrouping::Individual
                        || commits_to_consider == CommitsToConsider::All
                    {
                        let summary: GitCommitSummary = (&commit).try_into()?;
                        let mut proposed_issue_group = summary.clone();

                        // Use unique issue group names so each commit is
                        // addressable in the selection menu.
                        // DISCUSS: would it be better to use an array?
                        // No, because there's so much ambiguity. Should we expose the
                        // commit hash? Probably
                        while seen_issue_groups.contains(&proposed_issue_group) {
                            suffix += 1;
                            proposed_issue_group = GitCommitSummary(format!("{summary}_{suffix}"));
                        }

                        seen_issue_groups.insert(proposed_issue_group.clone());

                        return Ok(Some((IssueGroup::Commit(proposed_issue_group), commit)));
                    }

                    // Otherwise, skip this commit.
                    writeln!(
                        io::stderr(),
                        "Warning: ignoring commit without issue trailer: {:?}",
                        commit.id()
                    )?;
                    Ok(None)
                },
            )
            .filter_map(Result::transpose)
            .try_fold(
                Default::default(),
                |mut map,
                 maybe_tuple|
                 -> Result<IndexMap<IssueGroup, Vec<Commit>>, FromCommitsError> {
                    let (issue, commit) = maybe_tuple?;
                    let commits = map.entry(issue).or_default();
                    commits.push(commit);
                    Ok(map)
                },
            )?;

        Ok(Self(commits_by_issue))
    }

    pub fn select_issues(
        self,
        choose: PromptUserToChooseCommits,
        overlay: OverlayCommitsIntoOnePullRequest,
    ) -> Result<Self, SelectIssuesError> {
        let selected_issue_groups: IssueGroupWhitelist = {
            if choose == PromptUserToChooseCommits::No
                && overlay == OverlayCommitsIntoOnePullRequest::No
            {
                IssueGroupWhitelist::WhitelistDNE
            } else {
                let keys = self.0.keys();
                IssueGroupWhitelist::Whitelist(prompt_user(keys)?)
            }
        };

        Ok(match &selected_issue_groups {
            // If there is a whitelist, only operate on issue_groups in the whitelist
            IssueGroupWhitelist::Whitelist(whitelist) => self
                .into_iter()
                .filter(|(issue_group, _commits)| whitelist.contains(issue_group))
                .collect(),
            // If there is no whitelist, then operate on every issue
            IssueGroupWhitelist::WhitelistDNE => self,
        })
    }

    pub fn apply_overlay(self, overlay: OverlayCommitsIntoOnePullRequest) -> Self {
        match overlay {
            // If we are overlaying all active issue groups into one PR,
            // combine all active commits under the first issue group
            OverlayCommitsIntoOnePullRequest::Yes => self
                .into_iter()
                .reduce(|mut accumulator, mut item| {
                    accumulator.1.append(&mut item.1);
                    accumulator
                })
                // Map the option back into an IndexMap
                .map(|(issue_group, commits)| {
                    let mut map = Self::with_capacity(1);
                    map.insert(issue_group, commits);
                    map
                })
                .unwrap_or_default(),
            // If we are not overlaying issue groups, keep them separate
            OverlayCommitsIntoOnePullRequest::No => self,
        }
    }
}
