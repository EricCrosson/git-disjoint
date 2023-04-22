use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::anyhow;
use git2::Repository;
use git_url_parse::GitUrl;

pub(crate) struct GithubRepositoryMetadata {
    pub owner: String,
    // Terminology from https://stackoverflow.com/a/72018520
    // Best to clean that up
    pub forker: String,
    pub remote: String,
    pub name: String,
    pub root: PathBuf,
    pub repository: Repository,
}

impl GithubRepositoryMetadata {
    pub fn try_default() -> Result<Self, anyhow::Error> {
        let repo_root = get_repository_root()?;
        let repository = get_repository(&repo_root)?;
        let origin = get_remote_url("origin")?;
        let remote = get_user_remote(&repository)?;

        Ok(GithubRepositoryMetadata {
            owner: origin.owner.unwrap(),
            forker: get_remote_url(&remote)?.owner.unwrap(),
            remote,
            name: origin.name,
            root: get_repository_root()?,
            repository,
        })
    }
}

fn get_user_remote(repo: &Repository) -> Result<String, anyhow::Error> {
    let repo_remotes = repo.remotes()?;
    let mut remotes: HashSet<&str> = repo_remotes.iter().flatten().collect();

    remotes
        .take("fork")
        .or_else(|| remotes.take("origin"))
        .map(|str| str.to_owned())
        .ok_or_else(|| anyhow!("Unable to choose a git remote to push to, expected to find a remote named 'fork' or 'origin'"))
}

fn get_repository_root() -> Result<PathBuf, anyhow::Error> {
    let output_buffer = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?
        .stdout;
    let output = String::from_utf8(output_buffer)?.trim().to_owned();
    Ok(PathBuf::from(output))
}

fn get_repository(root: &Path) -> Result<Repository, anyhow::Error> {
    Ok(Repository::open(root)?)
}

fn get_remote_url(remote: &str) -> Result<GitUrl, anyhow::Error> {
    let output_buffer = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg(format!("remote.{remote}.url"))
        .output()?
        .stdout;
    let output = String::from_utf8(output_buffer)?.trim().to_owned();
    GitUrl::parse(&output)
        .map_err(|parse_error| anyhow!("Unable to parse origin remote url: {parse_error}"))
}
