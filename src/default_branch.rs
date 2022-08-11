use std::{process::Command, str::FromStr};

use anyhow::{Error, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct Repos {
    default_branch: String,
}

// DISCUSS: how can we downshift to a String automatically?
pub(crate) struct DefaultBranch(pub(crate) String);

impl From<String> for DefaultBranch {
    fn from(string: String) -> Self {
        DefaultBranch(string)
    }
}

impl FromStr for DefaultBranch {
    type Err = Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Ok(Self(string.to_owned()))
    }
}

impl DefaultBranch {
    pub(crate) fn try_get_default() -> Result<DefaultBranch> {
        // hub api /repos/{owner}/{repo}
        let stdout = String::from_utf8(
            Command::new("hub")
                .arg("api")
                .arg("/repos/{owner}/{repo}")
                .output()?
                .stdout,
        )?;

        let repos: Repos = serde_json::from_str(&stdout)?;

        Ok(repos.default_branch.into())
    }
}
