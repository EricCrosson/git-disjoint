use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};
use clap::Parser;
use git2::Repository;
use git_url_parse::GitUrl;

use crate::args::Args;
use crate::default_branch::DefaultBranch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitsToConsider {
    All,
    WithTrailer,
}

impl From<bool> for CommitsToConsider {
    fn from(value: bool) -> Self {
        match value {
            true => Self::All,
            false => Self::WithTrailer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptUserToChooseCommits {
    Yes,
    No,
}

impl From<bool> for PromptUserToChooseCommits {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Yes,
            false => Self::No,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OverlayCommitsIntoOnePullRequest {
    Yes,
    No,
}

impl From<bool> for OverlayCommitsIntoOnePullRequest {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Yes,
            false => Self::No,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitGrouping {
    Individual,
    ByIssue,
}

impl From<bool> for CommitGrouping {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Individual,
            false => Self::ByIssue,
        }
    }
}

pub(crate) struct GithubRepositoryMetadata {
    pub owner: String,
    // Terminology from https://stackoverflow.com/a/72018520
    // Best to clean that up
    pub forker: String,
    pub remote: String,
    pub name: String,
    pub root: PathBuf,
}

fn get_user_remote(repo: &Repository) -> Result<String> {
    let repo_remotes = repo.remotes()?;
    let mut remotes: HashSet<&str> = repo_remotes.iter().flatten().collect();

    remotes
        .take("fork")
        .or_else(|| remotes.take("origin"))
        .map(|str| str.to_owned())
        .ok_or_else(|| anyhow!("Unable to choose a git remote to push to, expected to find a remote named 'fork' or 'origin'"))
}

fn get_repository_root() -> Result<PathBuf> {
    let output_buffer = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?
        .stdout;
    let output = String::from_utf8(output_buffer)?.trim().to_owned();
    Ok(PathBuf::from(output))
}

fn get_repository(root: &Path) -> Result<Repository> {
    Ok(Repository::open(&root)?)
}

fn get_remote_url(remote: &str) -> Result<GitUrl> {
    let output_buffer = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg(format!("remote.{}.url", remote))
        .output()?
        .stdout;
    let output = String::from_utf8(output_buffer)?.trim().to_owned();
    GitUrl::parse(&output)
        .map_err(|parse_error| anyhow!("Unable to parse origin remote url: {parse_error}"))
}

fn get_repo_metadata(repository: &Repository) -> Result<GithubRepositoryMetadata> {
    let origin = get_remote_url("origin")?;
    let remote = get_user_remote(repository)?;

    Ok(GithubRepositoryMetadata {
        owner: origin.owner.unwrap(),
        forker: get_remote_url(&remote)?.owner.unwrap(),
        remote,
        name: origin.name,
        root: get_repository_root()?,
    })
}

pub(crate) struct SanitizedArgs {
    pub all: CommitsToConsider,
    pub base: DefaultBranch,
    pub choose: PromptUserToChooseCommits,
    pub dry_run: bool,
    pub github_token: String,
    pub overlay: OverlayCommitsIntoOnePullRequest,
    pub repository: Repository,
    pub repository_metadata: GithubRepositoryMetadata,
    pub separate: CommitGrouping,
}

impl SanitizedArgs {
    pub(crate) fn parse() -> Result<SanitizedArgs> {
        Args::parse().try_into()
    }
}

impl TryFrom<Args> for SanitizedArgs {
    type Error = anyhow::Error;

    fn try_from(value: Args) -> Result<Self, Self::Error> {
        let repo_root = get_repository_root()?;
        let repo = get_repository(&repo_root)?;
        let repository_metadata = get_repo_metadata(&repo)?;
        let Args {
            all,
            base,
            choose,
            dry_run,
            overlay,
            separate,
            github_token,
        } = value;
        Ok(Self {
            all: all.into(),
            // Clap doesn't provide a way to supply a default value coming from
            // a function when the user has not supplied a required value.
            // This TryFrom bridges the gap.
            base: base
                .map(DefaultBranch)
                .ok_or_else(|| anyhow!("User has not provided a default branch"))
                .or_else(|_| DefaultBranch::try_get_default(&repository_metadata, &github_token))?,
            choose: choose.into(),
            dry_run,
            github_token,
            overlay: overlay.into(),
            repository: repo,
            repository_metadata,
            separate: separate.into(),
        })
    }
}
