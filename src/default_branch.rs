use std::process::Command;

use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct Repos {
    default_branch: String,
}

// DISCUSS: how can we downshift to a String automatically?
#[derive(Clone, Debug)]
pub(crate) struct DefaultBranch(pub(crate) String);

impl DefaultBranch {
    pub(crate) fn parse(value: &str) -> Result<DefaultBranch> {
        Ok(DefaultBranch(value.to_owned()))
    }

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
