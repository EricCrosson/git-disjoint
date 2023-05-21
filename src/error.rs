//! The error type for top-level errors in git-disjoint.

use std::fmt::Display;

use crate::{execute::ExecuteError, pull_request};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct Error {
    pub kind: ErrorKind,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::PullRequest(_) => write!(f, "unable to create pull request"),
            ErrorKind::WebBrowser(_) => write!(f, "unable to open pull request in browser"),
            ErrorKind::CherryPick(_, commit) => {
                write!(f, "unable to cherry-pick commit {:?}", commit)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::PullRequest(err) => Some(err),
            ErrorKind::WebBrowser(err) => Some(err),
            ErrorKind::CherryPick(err, _) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    #[non_exhaustive]
    PullRequest(pull_request::CreatePullRequestError),
    #[non_exhaustive]
    WebBrowser(pull_request::CreatePullRequestError),
    #[non_exhaustive]
    CherryPick(ExecuteError, String),
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
