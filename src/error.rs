//! The error type for top-level errors in git-disjoint.

use std::fmt::Display;

use crate::{
    default_branch, disjoint_branch, editor, execute, git2_repository, github_repository_metadata,
    interact, issue_group_map, pull_request, pull_request_metadata,
};

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct Error {
    pub kind: ErrorKind,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::CreatePullRequest(_) => write!(f, "unable to create pull request"),
            ErrorKind::WebBrowser(_) => write!(f, "unable to open pull request in browser"),
            ErrorKind::CherryPick(_, commit) => {
                write!(f, "unable to cherry-pick commit {:?}", commit)
            }
            ErrorKind::RepositoryMetadata(_) => write!(f, "unable to gather repository metadata"),
            ErrorKind::DefaultBranch(_) => write!(f, "unable to query repository's default branch"),
            ErrorKind::BaseCommit(_) => write!(f, "unable to identify repository's base commit"),
            ErrorKind::WalkCommits(_) => write!(f, "unable to walk commits"),
            ErrorKind::IssueGroup(_) => write!(f, "unable to group commits by issue"),
            ErrorKind::SelectIssues(_) => write!(f, "unable to select issue groups"),
            ErrorKind::PlanBranches(_) => write!(f, "unable to plan commits onto branches"),
            ErrorKind::Git(_) => write!(f, "git operation failed"),
            ErrorKind::Execute(_) => write!(f, "command failed to execute"),
            ErrorKind::GetPullRequestMetadata(_) => {
                write!(f, "unable to query pull request metadata")
            }
            ErrorKind::ParsePullRequestMetadata(_) => {
                write!(f, "unable to parse pull request metadata")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ErrorKind::CreatePullRequest(err) => Some(err),
            ErrorKind::WebBrowser(err) => Some(err),
            ErrorKind::CherryPick(err, _) => Some(err),
            ErrorKind::RepositoryMetadata(err) => Some(err),
            ErrorKind::DefaultBranch(err) => Some(err),
            ErrorKind::BaseCommit(err) => Some(err),
            ErrorKind::WalkCommits(err) => Some(err),
            ErrorKind::IssueGroup(err) => Some(err),
            ErrorKind::SelectIssues(err) => Some(err),
            ErrorKind::PlanBranches(err) => Some(err),
            ErrorKind::Git(err) => Some(err),
            ErrorKind::Execute(err) => Some(err),
            ErrorKind::GetPullRequestMetadata(err) => Some(err),
            ErrorKind::ParsePullRequestMetadata(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum ErrorKind {
    #[non_exhaustive]
    RepositoryMetadata(github_repository_metadata::TryDefaultError),
    #[non_exhaustive]
    DefaultBranch(default_branch::TryDefaultError),
    #[non_exhaustive]
    BaseCommit(git2_repository::BaseCommitError),
    #[non_exhaustive]
    WalkCommits(git2_repository::WalkCommitsError),
    #[non_exhaustive]
    IssueGroup(issue_group_map::FromCommitsError),
    #[non_exhaustive]
    SelectIssues(interact::SelectIssuesError),
    #[non_exhaustive]
    PlanBranches(disjoint_branch::FromIssueGroupMapError),
    #[non_exhaustive]
    Git(git2::Error),
    #[non_exhaustive]
    Execute(execute::ExecuteError),
    #[non_exhaustive]
    GetPullRequestMetadata(editor::GetPullRequestMetadataError),
    #[non_exhaustive]
    ParsePullRequestMetadata(pull_request_metadata::FromStrError),
    #[non_exhaustive]
    CreatePullRequest(pull_request::CreatePullRequestError),
    #[non_exhaustive]
    WebBrowser(pull_request::CreatePullRequestError),
    #[non_exhaustive]
    CherryPick(execute::ExecuteError, String),
}

impl From<github_repository_metadata::TryDefaultError> for Error {
    fn from(err: github_repository_metadata::TryDefaultError) -> Self {
        Self {
            kind: ErrorKind::RepositoryMetadata(err),
        }
    }
}

impl From<default_branch::TryDefaultError> for Error {
    fn from(err: default_branch::TryDefaultError) -> Self {
        Self {
            kind: ErrorKind::DefaultBranch(err),
        }
    }
}

impl From<git2_repository::BaseCommitError> for Error {
    fn from(err: git2_repository::BaseCommitError) -> Self {
        Self {
            kind: ErrorKind::BaseCommit(err),
        }
    }
}

impl From<git2_repository::WalkCommitsError> for Error {
    fn from(err: git2_repository::WalkCommitsError) -> Self {
        Self {
            kind: ErrorKind::WalkCommits(err),
        }
    }
}

impl From<issue_group_map::FromCommitsError> for Error {
    fn from(err: issue_group_map::FromCommitsError) -> Self {
        Self {
            kind: ErrorKind::IssueGroup(err),
        }
    }
}

impl From<interact::SelectIssuesError> for Error {
    fn from(err: interact::SelectIssuesError) -> Self {
        Self {
            kind: ErrorKind::SelectIssues(err),
        }
    }
}

impl From<disjoint_branch::FromIssueGroupMapError> for Error {
    fn from(err: disjoint_branch::FromIssueGroupMapError) -> Self {
        Self {
            kind: ErrorKind::PlanBranches(err),
        }
    }
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        Self {
            kind: ErrorKind::Git(err),
        }
    }
}

impl From<execute::ExecuteError> for Error {
    fn from(err: execute::ExecuteError) -> Self {
        Self {
            kind: ErrorKind::Execute(err),
        }
    }
}

impl From<editor::GetPullRequestMetadataError> for Error {
    fn from(err: editor::GetPullRequestMetadataError) -> Self {
        Self {
            kind: ErrorKind::GetPullRequestMetadata(err),
        }
    }
}

impl From<pull_request_metadata::FromStrError> for Error {
    fn from(err: pull_request_metadata::FromStrError) -> Self {
        Self {
            kind: ErrorKind::ParsePullRequestMetadata(err),
        }
    }
}

impl From<pull_request::CreatePullRequestError> for Error {
    fn from(err: pull_request::CreatePullRequestError) -> Self {
        match &err.kind {
            pull_request::CreatePullRequestErrorKind::Http(_)
            | pull_request::CreatePullRequestErrorKind::Parse(_) => Self {
                kind: ErrorKind::CreatePullRequest(err),
            },
            pull_request::CreatePullRequestErrorKind::OpenBrowser(_) => Self {
                kind: ErrorKind::WebBrowser(err),
            },
        }
    }
}
