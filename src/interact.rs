use std::{collections::HashSet, error::Error, fmt::Display};

use git2::Commit;
use indexmap::IndexMap;
use inquire::{formatter::MultiOptionFormatter, MultiSelect};

use crate::{
    cli::{OverlayCommitsIntoOnePullRequest, PromptUserToChooseCommits},
    issue_group::IssueGroup,
};

#[derive(Debug)]
pub(crate) enum IssueGroupWhitelist {
    Whitelist(HashSet<IssueGroup>),
    WhitelistDNE,
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct SelectIssuesError {
    kind: SelectIssuesErrorKind,
}

impl Display for SelectIssuesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            SelectIssuesErrorKind::Prompt(_) => write!(f, "unable to process issue selection"),
        }
    }
}

impl Error for SelectIssuesError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            SelectIssuesErrorKind::Prompt(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum SelectIssuesErrorKind {
    #[non_exhaustive]
    Prompt(inquire::InquireError),
}

impl From<inquire::InquireError> for SelectIssuesError {
    fn from(err: inquire::InquireError) -> Self {
        Self {
            kind: SelectIssuesErrorKind::Prompt(err),
        }
    }
}

fn prompt_user(choices: Vec<&IssueGroup>) -> Result<HashSet<IssueGroup>, SelectIssuesError> {
    let formatter: MultiOptionFormatter<&IssueGroup> =
        &|selected| format!("Selected: {selected:?}");

    Ok(
        MultiSelect::new("Select the issues to create PRs for:", choices)
            .with_formatter(formatter)
            .prompt()?
            .into_iter()
            .cloned()
            .collect(),
    )
}

pub(crate) fn select_issues(
    commits_by_issue_group: &IndexMap<IssueGroup, Vec<Commit>>,
    choose: PromptUserToChooseCommits,
    overlay: OverlayCommitsIntoOnePullRequest,
) -> Result<IssueGroupWhitelist, SelectIssuesError> {
    if choose == PromptUserToChooseCommits::No && overlay == OverlayCommitsIntoOnePullRequest::No {
        return Ok(IssueGroupWhitelist::WhitelistDNE);
    }

    let keys = commits_by_issue_group.keys().collect();
    Ok(IssueGroupWhitelist::Whitelist(prompt_user(keys)?))
}
