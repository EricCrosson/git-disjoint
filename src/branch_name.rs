use std::{error::Error, fmt::Display};

use sanitize_git_ref::sanitize_git_ref_onelevel;

use crate::{increment::Increment, issue_group::IssueGroup};

/// Characters to be replaced with a hyphen, since they interfere with terminal
/// tab-completion.
static CHARACTERS_TO_REPLACE_WITH_HYPHEN: &[char] = &['!', '`', '(', ')'];

/// Characters to be deleted, since they interfere with terminal tab-completion.
static CHARACTERS_TO_REMOVE: &[char] = &['\'', '"'];

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct BranchName(String);

fn elide_consecutive_hyphens(mut s: String) -> String {
    let mut current_run = 0;
    s.retain(|c| {
        match c == '-' {
            true => current_run += 1,
            false => current_run = 0,
        };
        current_run < 2
    });
    s
}

impl BranchName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_issue_group(issue_group: &IssueGroup, summary: &str) -> Self {
        let raw_branch_name = match issue_group {
            IssueGroup::Issue(issue_group) => format!(
                "{}-{}",
                issue_group.issue_identifier(),
                summary.to_lowercase()
            ),
            IssueGroup::Commit(summary) => summary.0.clone().to_lowercase(),
        };
        Self::new(sanitize_git_ref_onelevel(&raw_branch_name))
    }

    pub fn new(value: String) -> Self {
        let s = value.replace(CHARACTERS_TO_REPLACE_WITH_HYPHEN, "-");
        let s = elide_consecutive_hyphens(s);
        let s = s.replace(CHARACTERS_TO_REMOVE, "");
        Self(s)
    }
}

impl Display for BranchName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for BranchName {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[derive(Debug)]
pub(crate) enum IncrementError {
    #[non_exhaustive]
    Overflow,
}

impl Display for IncrementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "counter addition overflowed")
    }
}

impl Error for IncrementError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl Increment for BranchName {
    type Error = IncrementError;

    fn increment(self, count: u32) -> Result<(Self, u32), Self::Error> {
        let next = count.checked_add(1).ok_or(IncrementError::Overflow)?;
        let incremented = Self(format!("{}_{}", self.0, next));
        Ok((incremented, next))
    }
}
