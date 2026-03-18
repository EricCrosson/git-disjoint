use std::fmt::Display;

use sanitize_git_ref::sanitize_git_ref_onelevel;

use crate::issue_group::IssueGroup;

/// Characters to be replaced with a hyphen, since they interfere with terminal
/// tab-completion.
static CHARACTERS_TO_REPLACE_WITH_HYPHEN: &[char] = &['!', '`', '(', ')'];

/// Characters to be deleted, since they interfere with terminal tab-completion.
static CHARACTERS_TO_REMOVE: &[char] = &['\'', '"'];

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BranchName(String);

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
        let s = s.replace(CHARACTERS_TO_REMOVE, "");
        let s = elide_consecutive_hyphens(s);
        let s = s.trim_matches('-').to_string();
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

#[cfg(test)]
mod test {
    use super::BranchName;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn sanitization_is_idempotent(s in "\\PC*") {
            let once = BranchName::new(s.clone());
            let twice = BranchName::new(once.as_str().to_string());
            prop_assert_eq!(once, twice);
        }

        #[test]
        fn no_consecutive_hyphens(s in "\\PC*") {
            let branch = BranchName::new(s);
            prop_assert!(!branch.as_str().contains("--"),
                "branch name contains consecutive hyphens: {:?}", branch.as_str());
        }

        #[test]
        fn no_leading_or_trailing_hyphens(s in "\\PC*") {
            let branch = BranchName::new(s);
            let name = branch.as_str();
            if !name.is_empty() {
                prop_assert!(!name.starts_with('-'),
                    "branch name starts with hyphen: {:?}", name);
                prop_assert!(!name.ends_with('-'),
                    "branch name ends with hyphen: {:?}", name);
            }
        }

        #[test]
        fn no_forbidden_chars(s in "\\PC*") {
            let branch = BranchName::new(s);
            let name = branch.as_str();
            prop_assert!(!name.contains('('), "contains '('");
            prop_assert!(!name.contains(')'), "contains ')'");
            prop_assert!(!name.contains('`'), "contains backtick");
            prop_assert!(!name.contains('\''), "contains single quote");
            prop_assert!(!name.contains('"'), "contains double quote");
        }
    }
}
