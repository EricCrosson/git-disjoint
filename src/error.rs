//! The error type for top-level errors in git-disjoint.

use std::fmt::Display;

use crate::pull_request;

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct Error {
    kind: ErrorKind,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::PullRequest(_) => write!(f, "unable to create pull request"),
            ErrorKind::WebBrowser(_) => write!(f, "unable to open pull request in browser"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::PullRequest(err) => Some(err),
            ErrorKind::WebBrowser(_) => todo!(),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    #[non_exhaustive]
    PullRequest(pull_request::CreatePullRequestError),
    #[non_exhaustive]
    WebBrowser(pull_request::CreatePullRequestError),
}

impl From<pull_request::CreatePullRequestError> for Error {
    fn from(err: pull_request::CreatePullRequestError) -> Self {
        match &err.kind {
            pull_request::CreatePullRequestErrorKind::Http(_)
            | pull_request::CreatePullRequestErrorKind::Parse(_) => Self {
                kind: ErrorKind::PullRequest(err),
            },
            pull_request::CreatePullRequestErrorKind::OpenBrowser(_) => Self {
                kind: ErrorKind::WebBrowser(err),
            },
        }
    }
}
