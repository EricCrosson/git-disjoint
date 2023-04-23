use std::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    static ref RE_JIRA_ISSUE: Regex = Regex::new(r"(?m)^(?:Closes )?Ticket:\s+(\S+)")
        .expect("Expected regular expression to compile");
    static ref RE_GITHUB_ISSUE: Regex =
        Regex::new(r"(?im)^(?:closes|close|closed|fixes|fixed)\s+#(\d+)")
            .expect("Expected regular expression to compile");
}

/// Jira or GitHub issue identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub(crate) enum Issue {
    Jira(String),
    GitHub(String),
}

impl Issue {
    pub fn parse_from_commit_message<S: AsRef<str>>(commit_message: S) -> Option<Issue> {
        if let Some(jira_captures) = RE_JIRA_ISSUE.captures(commit_message.as_ref()) {
            return Some(Issue::Jira(
                jira_captures[jira_captures.len() - 1].to_owned(),
            ));
        }
        if let Some(github_captures) = RE_GITHUB_ISSUE.captures(commit_message.as_ref()) {
            return Some(Issue::GitHub(
                github_captures[github_captures.len() - 1].to_owned(),
            ));
        }
        None
    }

    pub fn issue_identifier(&self) -> &str {
        match self {
            Issue::Jira(ticket) => ticket,
            Issue::GitHub(issue) => issue,
        }
    }
}

impl Display for Issue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            match self {
                Issue::Jira(_) => "Jira ",
                Issue::GitHub(_) => "GitHub #",
            },
            self.issue_identifier()
        )
    }
}

#[cfg(test)]
mod test {
    use indoc::formatdoc;

    use super::Issue;

    #[test]
    fn display_jira_issue() {
        let issue = Issue::Jira("GD-0".to_string());
        assert_eq!(format!("{issue}"), "Jira GD-0");
    }

    #[test]
    fn display_github_issue() {
        let issue = Issue::GitHub("123".to_string());
        assert_eq!(format!("{issue}"), "GitHub #123");
    }

    macro_rules! test_parses {
        ($unit_test:ident, $input:expr, $output:expr) => {
            #[test]
            fn $unit_test() {
                let message = $input;
                let issue = Issue::parse_from_commit_message(message);
                assert!(
                    issue.is_some(),
                    "Expected to parse issue from commit message"
                );
                let issue = issue.unwrap();
                assert_eq!(issue, $output);
            }
        };
    }

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Ticket: AB-123     
            "
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive
        
            Ticket: AB-123
        
            ",
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_trailer,
        formatdoc!(
            "
            feat(foo): add hyperdrive
        
            Ticket: AB-123
            Footer: http://example.com
            ",
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123
            ",
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123

            ",
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_trailer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123
            Footer: http://example.com
            ",
        ),
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            ",
        ),
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            "
        ),
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_trailer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            Footer: http://example.com
            ",
        ),
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            ",
        ),
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123

            ",
        ),
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_trailer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            Footer: http://example.com
            ",
        ),
        Issue::GitHub("123".to_string())
    );

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
