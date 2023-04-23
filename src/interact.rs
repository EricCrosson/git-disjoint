use std::collections::HashSet;

use anyhow::anyhow;
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

fn prompt_user(choices: Vec<&IssueGroup>) -> Result<HashSet<IssueGroup>, anyhow::Error> {
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
) -> Result<IssueGroupWhitelist, anyhow::Error> {
    if choose == PromptUserToChooseCommits::No && overlay == OverlayCommitsIntoOnePullRequest::No {
        return Ok(IssueGroupWhitelist::WhitelistDNE);
    }

    let keys = commits_by_issue_group.keys().collect();
    Ok(IssueGroupWhitelist::Whitelist(prompt_user(keys).map_err(
        |_| anyhow!("Unable to process issue selection"),
    )?))
}
