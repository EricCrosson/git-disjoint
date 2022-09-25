use std::collections::HashSet;

use anyhow::{anyhow, Result};
use git2::Repository;

pub(crate) struct UserConfig {
    pub remote: String,
}

pub(crate) fn get_user_remote(repo: &Repository) -> Result<String> {
    let repo_remotes = repo.remotes()?;
    let mut remotes: HashSet<&str> = repo_remotes.iter().flatten().collect();

    remotes
        .take("fork")
        .or_else(|| remotes.take("origin"))
        .map(|str| str.to_owned())
        .ok_or_else(|| anyhow!("Unable to choose a git remote to push to, expected to find a remote named 'fork' or 'origin'"))
}
