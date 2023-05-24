use std::{error::Error, fmt::Display, io};

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{branch_name::BranchName, default_branch::DefaultBranch};

#[derive(Debug)]
pub(crate) struct PullRequest {
    pub owner: String,
    pub name: String,
    pub forker: String,
    pub title: String,
    pub body: String,
    pub github_token: String,
    pub branch_name: BranchName,
    pub base: DefaultBranch,
}

// https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#create-a-pull-request
#[derive(Debug, Serialize)]
struct CreatePullRequestRequest {
    title: String,
    body: String,
    head: String,
    base: String,
    draft: bool,
}

// https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#create-a-pull-request
#[derive(Debug, Deserialize)]
struct CreatePullRequestResponse {
    html_url: String,
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct CreatePullRequestError {
    url: String,
    pub kind: CreatePullRequestErrorKind,
}

impl Display for CreatePullRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            CreatePullRequestErrorKind::Http(_) => write!(f, "http error: POST {}", self.url),
            CreatePullRequestErrorKind::Parse(_) => {
                write!(f, "unable to parse response from POST {}", self.url)
            }
            CreatePullRequestErrorKind::OpenBrowser(_) => {
                write!(f, "unable to open web browser to page {}", self.url)
            }
        }
    }
}

impl Error for CreatePullRequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            CreatePullRequestErrorKind::Http(err) => Some(err),
            CreatePullRequestErrorKind::Parse(err) => Some(err),
            CreatePullRequestErrorKind::OpenBrowser(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum CreatePullRequestErrorKind {
    #[non_exhaustive]
    Http(reqwest::Error),
    #[non_exhaustive]
    Parse(reqwest::Error),
    #[non_exhaustive]
    OpenBrowser(io::Error),
}

impl PullRequest {
    pub async fn create(self, http_client: Client) -> Result<(), CreatePullRequestError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            self.owner, self.name
        );
        let response: CreatePullRequestResponse = http_client
            .post(&url)
            .header("User-Agent", "git-disjoint")
            .header("Accept", "application/vnd.github.v3+json")
            .header("Authorization", format!("token {}", self.github_token))
            .json(&CreatePullRequestRequest {
                title: self.title,
                body: self.body,
                head: format!("{}:{}", self.forker, self.branch_name),
                base: self.base.0,
                draft: true,
            })
            .send()
            .await
            .map_err(|err| CreatePullRequestError {
                url: url.clone(),
                kind: CreatePullRequestErrorKind::Http(err),
            })?
            .json()
            .await
            .map_err(|err| CreatePullRequestError {
                url: url.clone(),
                kind: CreatePullRequestErrorKind::Parse(err),
            })?;

        let url = response.html_url;
        open::that(&url).map_err(|err| CreatePullRequestError {
            url,
            kind: CreatePullRequestErrorKind::OpenBrowser(err),
        })?;

        Ok(())
    }
}
