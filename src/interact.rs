use std::collections::HashSet;

use anyhow::Result;
use inquire::{formatter::MultiOptionFormatter, MultiSelect};

use crate::issue::Issue;

pub(crate) fn select_issues(from: Vec<&Issue>) -> Result<HashSet<Issue>> {
    let formatter: MultiOptionFormatter<&Issue> = &|selected| format!("Selected: {:?}:", selected);

    Ok(
        MultiSelect::new("Select the issues to create PRs for:", from)
            .with_formatter(formatter)
            .prompt()?
            .into_iter()
            .cloned()
            .collect(),
    )
}
