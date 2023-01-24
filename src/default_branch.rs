use std::str::FromStr;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::sanitized_args::GithubRepositoryMetadata;

#[derive(Clone, Debug)]
pub(crate) struct DefaultBranch(pub String);

#[derive(Debug, Deserialize)]
struct GetRepositoryResponse {
    default_branch: String,
}

impl DefaultBranch {
    pub(crate) fn try_get_default(
        repository_metadata: &GithubRepositoryMetadata,
        github_token: &str,
    ) -> Result<DefaultBranch> {
        let http_client = reqwest::blocking::Client::new();
        let response: GetRepositoryResponse = http_client
            .get(format!(
                "https://api.github.com/repos/{}/{}",
                repository_metadata.owner, repository_metadata.name
            ))
            .header("User-Agent", "git-disjoint")
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", format!("token {}", github_token))
            .send()
            .map_err(|request_error| anyhow!("Error contacting the GitHub API: {request_error}"))?
            .json()
            .map_err(|response_error| {
                anyhow!("Error parsing the GitHub API response: {response_error}")
            })?;

        // Assumption: `origin` is the upstream/main repositiory
        Ok(DefaultBranch(response.default_branch))
    }
}
