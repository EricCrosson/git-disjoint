use std::{error::Error, fmt::Display};

use serde::Deserialize;

use crate::github_repository_metadata::GithubRepositoryMetadata;

#[derive(Clone, Debug)]
pub(crate) struct DefaultBranch(pub String);

#[derive(Debug, Deserialize)]
struct GetRepositoryResponse {
    default_branch: String,
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct TryDefaultError {
    // owner/name repository slug
    url: String,
    kind: TryDefaultErrorKind,
}

impl Display for TryDefaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            TryDefaultErrorKind::Http(_) => write!(f, "http error: GET {}", self.url),
            TryDefaultErrorKind::Parse(_) => {
                write!(f, "unable to parse response from GET {}", self.url)
            }
        }
    }
}

impl Error for TryDefaultError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            TryDefaultErrorKind::Http(err) => Some(err),
            TryDefaultErrorKind::Parse(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum TryDefaultErrorKind {
    #[non_exhaustive]
    Http(reqwest::Error),
    #[non_exhaustive]
    Parse(reqwest::Error),
}

impl DefaultBranch {
    pub(crate) async fn try_get_default(
        repository_metadata: &GithubRepositoryMetadata,
        github_token: &str,
    ) -> Result<DefaultBranch, TryDefaultError> {
        let http_client = reqwest::Client::new();
        let url = format!(
            "https://api.github.com/repos/{}/{}",
            repository_metadata.owner, repository_metadata.name
        );
        let response: GetRepositoryResponse = http_client
            .get(&url)
            .header("User-Agent", "git-disjoint")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("token {github_token}"))
            .send()
            .await
            .map_err(|err| TryDefaultError {
                url: url.clone(),
                kind: TryDefaultErrorKind::Http(err),
            })?
            .json()
            .await
            .map_err(|err| TryDefaultError {
                url,
                kind: TryDefaultErrorKind::Parse(err),
            })?;

        // Assumption: `origin` is the upstream/main repositiory
        Ok(DefaultBranch(response.default_branch))
    }
}
