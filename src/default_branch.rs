use std::{process::Command, str::FromStr};

use anyhow::{Context, Result};
use indoc::formatdoc;
use serde::Deserialize;

#[derive(Deserialize)]
struct Repos {
    default_branch: String,
}

#[derive(Clone, Debug)]
pub(crate) struct DefaultBranch(pub String);

impl FromStr for DefaultBranch {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl DefaultBranch {
    pub(crate) fn try_get_default() -> Result<DefaultBranch> {
        let stdout = String::from_utf8(
            Command::new("hub")
                .arg("api")
                .arg("/repos/{owner}/{repo}")
                .output()?
                .stdout,
        )?;

        let repos: Repos = serde_json::from_str(&stdout).with_context(|| {
            formatdoc!(
                "
                Unable to query the repository's default branch from the GitHub API.
                
                Do you have hub configured? You should be able to run
                
                ```
                hub api /repos/{{owner}}/{{repo}}
                ```
                
                without error.
                "
            )
        })?;

        // Assumption: `origin` is the upstream/main repositiory
        Ok(DefaultBranch(format!("origin/{}", repos.default_branch)))
    }
}
