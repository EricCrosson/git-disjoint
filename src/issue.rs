use std::fmt::Display;

use lazy_static::lazy_static;
use regex::Regex;

#[cfg(test)]
use proptest_derive::Arbitrary;

lazy_static! {
    static ref RE_JIRA_ISSUE: Regex = Regex::new(r"(?m)^(?:Closes )?Ticket:\s+(\S+)")
        .expect("Expected regular expression to compile");
}
lazy_static! {
    static ref RE_GITHUB_ISSUE: Regex =
        Regex::new(r"(?im)^(closes|close|closed|fixes|fixed)\s+#(\d+)")
            .expect("Expected regular expression to compile");
}

/// Jira or GitHub issue identifier.
#[derive(Debug, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub(crate) struct Issue(String);

impl Issue {
    pub(crate) fn parse_from_commit_message<S: AsRef<str>>(commit_message: S) -> Option<Issue> {
        let captures = RE_JIRA_ISSUE
            .captures(commit_message.as_ref())
            .or_else(|| RE_GITHUB_ISSUE.captures(commit_message.as_ref()))?;
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
    use indoc::formatdoc;

    use crate::issue::Issue;

    #[test]
    fn display_issue() {
        let issue = Issue(String::from("GD-0"));
        assert_eq!(format!("{}", issue), "GD-0");
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
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive
        
            Ticket: AB-123
        
            ",
        ),
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_footer,
        formatdoc!(
            "
            feat(foo): add hyperdrive
        
            Ticket: AB-123
            Footer: http://example.com
            ",
        ),
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123
            ",
        ),
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123

            ",
        ),
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_footer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes Ticket: AB-123
            Footer: http://example.com
            ",
        ),
        Issue(String::from("AB-123"))
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            ",
        ),
        Issue(String::from("123"))
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            "
        ),
        Issue(String::from("123"))
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_footer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            Footer: http://example.com
            ",
        ),
        Issue(String::from("123"))
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_without_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            ",
        ),
        Issue(String::from("123"))
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_newline,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123

            ",
        ),
        Issue(String::from("123"))
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_footer,
        formatdoc!(
            "
            feat(foo): add hyperdrive

            Closes #123
            Footer: http://example.com
            ",
        ),
        Issue(String::from("123"))
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
