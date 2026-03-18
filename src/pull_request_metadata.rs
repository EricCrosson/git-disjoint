use std::{error::Error, fmt::Display, str::FromStr};

use crate::pull_request_message::IGNORE_MARKER;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PullRequestMetadata {
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct FromStrError {
    kind: FromStrErrorKind,
}

impl Display for FromStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FromStrErrorKind::EmptyPullRequestMessage => {
                write!(f, "pull request metadata is empty")
            }
        }
    }
}

impl Error for FromStrError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FromStrErrorKind::EmptyPullRequestMessage => None,
        }
    }
}

#[derive(Debug)]
pub enum FromStrErrorKind {
    #[non_exhaustive]
    EmptyPullRequestMessage,
}

impl From<FromStrErrorKind> for FromStrError {
    fn from(kind: FromStrErrorKind) -> Self {
        Self { kind }
    }
}

impl FromStr for PullRequestMetadata {
    type Err = FromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(FromStrErrorKind::EmptyPullRequestMessage)?;
        }

        let mut iterator = s.lines();
        let title = iterator.next().unwrap_or_default().trim().to_owned();
        let body = iterator
            .take_while(|line| line != &IGNORE_MARKER)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_owned();

        Ok(Self { title, body })
    }
}

#[cfg(test)]
mod test {
    use super::PullRequestMetadata;
    use crate::pull_request_message::IGNORE_MARKER;

    #[test]
    fn parse_empty_string_returns_error() {
        let result = "".parse::<PullRequestMetadata>();
        assert!(result.is_err());
    }

    #[test]
    fn parse_title_only() {
        let result = "My PR title".parse::<PullRequestMetadata>().unwrap();
        assert_eq!(result.title, "My PR title");
        assert_eq!(result.body, "");
    }

    #[test]
    fn parse_title_with_body() {
        let result = "Title\n\nBody paragraph"
            .parse::<PullRequestMetadata>()
            .unwrap();
        assert_eq!(result.title, "Title");
        assert_eq!(result.body, "Body paragraph");
    }

    #[test]
    fn parse_strips_content_after_ignore_marker() {
        let input = format!("Title\n\nBody\n{IGNORE_MARKER}\nshould be ignored");
        let result = input.parse::<PullRequestMetadata>().unwrap();
        assert_eq!(result.title, "Title");
        assert_eq!(result.body, "Body");
    }

    #[test]
    fn parse_trims_whitespace() {
        let result = "  Title with spaces  \n\n  Body with spaces  "
            .parse::<PullRequestMetadata>()
            .unwrap();
        assert_eq!(result.title, "Title with spaces");
        assert_eq!(result.body, "Body with spaces");
    }
}
