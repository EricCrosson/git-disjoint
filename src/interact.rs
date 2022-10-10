use std::collections::HashSet;

use anyhow::Result;
use inquire::{formatter::MultiOptionFormatter, MultiSelect};

use crate::issue_group::IssueGroup;

pub(crate) fn select_issues(from: Vec<&IssueGroup>) -> Result<HashSet<IssueGroup>> {
    let formatter: MultiOptionFormatter<&IssueGroup> =
        &|selected| format!("Selected: {:?}", selected);

    Ok(
        MultiSelect::new("Select the issues to create PRs for:", from)
            .with_formatter(formatter)
            .prompt()?
            .into_iter()
            .cloned()
            .collect(),
    )
}
