use std::{process::Command, str::FromStr};

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Repos {
    default_branch: String,
}

// DISCUSS: how can we downshift to a String automatically?
#[derive(Clone, Debug)]
pub(crate) struct DefaultBranch(pub(crate) String);

impl FromStr for DefaultBranch {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
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

        Ok(DefaultBranch(repos.default_branch))
    }
}
