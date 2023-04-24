use std::{error::Error, fmt::Display, str::FromStr};

use crate::pull_request_message::IGNORE_MARKER;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct PullRequestMetadata {
    pub title: String,
    pub body: String,
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct FromStrError {
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
pub(crate) enum FromStrErrorKind {
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
