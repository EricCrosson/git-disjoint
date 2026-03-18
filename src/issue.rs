use std::fmt::Display;
use std::sync::OnceLock;

macro_rules! regex {
    ($re:literal $(,)?) => {{
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($re).unwrap())
    }};
}

/// Jira or GitHub issue identifier.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(test, derive(proptest_derive::Arbitrary))]
pub enum Issue {
    Jira(String),
    GitHub(String),
}

impl Issue {
    pub fn parse_from_commit_message<S: AsRef<str>>(commit_message: S) -> Option<Issue> {
        let regex_jira_issue = regex!(r"(?m)^(?:Closes )?Ticket:\s+(\S+)");
        if let Some(jira_captures) = regex_jira_issue.captures(commit_message.as_ref()) {
            return Some(Issue::Jira(
                jira_captures[jira_captures.len() - 1].to_owned(),
            ));
        }

        let regex_github_issue = regex!(r"(?im)^(?:closes|close|closed|fixes|fixed)\s+#(\d+)");
        if let Some(github_captures) = regex_github_issue.captures(commit_message.as_ref()) {
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
        r#"
feat(foo): add hyperdrive

Ticket: AB-123     
"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_newline,
        r#"
feat(foo): add hyperdrive

Ticket: AB-123
        
"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_from_commit_message_with_trailer,
        r#"
feat(foo): add hyperdrive

Ticket: AB-123
Footer: http://example.com
"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_without_newline,
        r#"
feat(foo): add hyperdrive

Closes Ticket: AB-123
"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_newline,
        r#"
feat(foo): add hyperdrive

Closes Ticket: AB-123

"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_jira_ticket_closes_ticket_from_commit_message_with_trailer,
        r#"
feat(foo): add hyperdrive

Closes Ticket: AB-123
Footer: http://example.com
"#,
        Issue::Jira("AB-123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_without_newline,
        r#"
feat(foo): add hyperdrive

Closes #123
"#,
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_newline,
        r#"
feat(foo): add hyperdrive

Closes #123
"#,
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_from_commit_message_with_trailer,
        r#"
feat(foo): add hyperdrive

Closes #123
            Footer: http://example.com
"#,
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_without_newline,
        r#"
feat(foo): add hyperdrive

Closes #123
"#,
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_newline,
        r#"
feat(foo): add hyperdrive

Closes #123

"#,
        Issue::GitHub("123".to_string())
    );

    test_parses!(
        successfully_parse_github_issue_closes_ticket_from_commit_message_with_trailer,
        r#"
feat(foo): add hyperdrive

Closes #123
            Footer: http://example.com
"#,
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

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn display_is_never_empty(issue in prop_oneof![
            "[A-Z]{2,6}-[0-9]{1,6}".prop_map(Issue::Jira),
            "[0-9]{1,6}".prop_map(Issue::GitHub),
        ]) {
            let display = format!("{}", issue);
            prop_assert!(!display.is_empty());
        }

        #[test]
        fn display_then_parse_roundtrip_jira(
            project in "[A-Z]{2,6}",
            number in 1u32..=99999u32,
        ) {
            let id = format!("{project}-{number}");
            let message = format!("feat: x\n\nTicket: {id}");
            let parsed = Issue::parse_from_commit_message(&message);
            prop_assert_eq!(parsed, Some(Issue::Jira(id)));
        }

        #[test]
        fn display_then_parse_roundtrip_github(number in 1u32..=99999u32) {
            let message = format!("fix: x\n\nCloses #{number}");
            let parsed = Issue::parse_from_commit_message(&message);
            prop_assert_eq!(parsed, Some(Issue::GitHub(number.to_string())));
        }
    }
}
