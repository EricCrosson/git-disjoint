use std::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;

#[cfg(test)]
use proptest_derive::Arbitrary;

/// Jira issue identifier.
#[derive(Debug, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub(crate) struct Issue(String);

impl Issue {
    pub(crate) fn parse_from_commit_message<S: AsRef<str>>(commit_message: S) -> Option<Issue> {
        lazy_static! {
            static ref RE_JIRA_ISSUE: Regex = Regex::new(r"(?m)^Ticket:\s+(\S+)")
                .expect("Expected regular expression to compile");
        }
        let captures = RE_JIRA_ISSUE.captures(commit_message.as_ref())?;
        Some(Issue(captures[captures.len() - 1].to_owned()))
    }
}

impl Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod test {
    use crate::issue::Issue;

    #[test]
    fn display_issue() {
        let issue = Issue(String::from("GD-0"));
        assert_eq!(format!("{}", issue), "GD-0");
    }

    #[test]
    fn successfully_parse_from_commit_message_without_newline() {
        let message = r#"feat(foo): add hyperdrive
        
Ticket: AB-123"#;
        let issue = Issue::parse_from_commit_message(message);
        assert!(
            issue.is_some(),
            "Expected to parse issue from commit message"
        );
        let issue = issue.unwrap();
        assert_eq!(issue, Issue(String::from("AB-123")));
    }

    #[test]
    fn successfully_parse_from_commit_message_with_newline() {
        let message = r#"feat(foo): add hyperdrive
        
Ticket: AB-123
        "#;
        let issue = Issue::parse_from_commit_message(message);
        assert!(
            issue.is_some(),
            "Expected to parse issue from commit message"
        );
        let issue = issue.unwrap();
        assert_eq!(issue, Issue(String::from("AB-123")));
    }

    #[test]
    fn successfully_parse_from_commit_message_with_footer() {
        let message = r#"feat(foo): add hyperdrive
        
Ticket: AB-123
Footer: http://example.com"#;
        let issue = Issue::parse_from_commit_message(message);
        assert!(
            issue.is_some(),
            "Expected to parse issue from commit message"
        );
        let issue = issue.unwrap();
        assert_eq!(issue, Issue(String::from("AB-123")));
    }

    #[test]
    fn unnsuccessfully_parse_from_commit_message() {
        let message = "feat(foo): add hyperdrive";
        let issue = Issue::parse_from_commit_message(message);
        assert!(
            issue.is_none(),
            "Expected to find no issue to parse from commit message"
        );
    }
}
