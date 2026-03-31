use std::{error::Error, fmt::Display, io};

use serde::{Deserialize, Serialize};

use crate::{branch_name::BranchName, default_branch::DefaultBranch};

#[derive(Debug)]
pub struct PullRequest {
    pub owner: String,
    pub name: String,
    pub forker: String,
    pub title: String,
    pub body: String,
    pub github_token: String,
    pub branch_name: BranchName,
    pub base: DefaultBranch,
    pub draft: bool,
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
pub struct CreatePullRequestError {
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
pub enum CreatePullRequestErrorKind {
    #[non_exhaustive]
    Http(reqwest::Error),
    #[non_exhaustive]
    Parse(reqwest::Error),
    #[non_exhaustive]
    OpenBrowser(io::Error),
}

impl PullRequest {
    fn build_request(&self) -> CreatePullRequestRequest {
        CreatePullRequestRequest {
            title: self.title.clone(),
            body: self.body.clone(),
            head: format!("{}:{}", self.forker, self.branch_name),
            base: self.base.0.clone(),
            draft: self.draft,
        }
    }

    pub fn create(
        self,
        http_client: reqwest::blocking::Client,
    ) -> Result<(), CreatePullRequestError> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls",
            self.owner, self.name
        );
        let response: CreatePullRequestResponse = http_client
            .post(&url)
            .header("User-Agent", "git-disjoint")
            .header("Accept", "application/vnd.github.v3+json")
            .header("Authorization", format!("token {}", self.github_token))
            .json(&self.build_request())
            .send()
            .map_err(|err| CreatePullRequestError {
                url: url.clone(),
                kind: CreatePullRequestErrorKind::Http(err),
            })?
            .json()
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{branch_name::BranchName, default_branch::DefaultBranch};

    fn test_pull_request(draft: bool) -> PullRequest {
        PullRequest {
            owner: "owner".into(),
            name: "repo".into(),
            forker: "forker".into(),
            title: "Fix the widget".into(),
            body: "This fixes the broken widget.\n\nTicket: PROJ-123".into(),
            github_token: "token".into(),
            branch_name: BranchName::new("proj-123-fix-the-widget".into()),
            base: DefaultBranch("main".into()),
            draft,
        }
    }

    #[test]
    fn build_request_creates_draft_pr() {
        let pr = test_pull_request(true);
        let req = pr.build_request();
        insta::assert_snapshot!(serde_json::to_string_pretty(&req).unwrap());
    }

    #[test]
    fn build_request_creates_ready_pr() {
        let pr = test_pull_request(false);
        let req = pr.build_request();
        insta::assert_snapshot!(serde_json::to_string_pretty(&req).unwrap());
    }
}
