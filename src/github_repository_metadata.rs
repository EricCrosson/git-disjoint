use std::{
    collections::HashSet, error::Error, fmt::Display, io, path::PathBuf, process::Command,
    string::FromUtf8Error,
};

use git_url_parse::GitUrl;

use crate::git2_repository::{self, Repository};

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

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct TryDefaultError {
    kind: TryDefaultErrorKind,
}

impl Display for TryDefaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            TryDefaultErrorKind::RunCommand(_) => write!(f, "error running command"),
            TryDefaultErrorKind::ParseCommandOutput(_) => write!(f, "command output contains invalid UTF-8"),
            TryDefaultErrorKind::OpenRepository(_) => write!(f, "unable to open git repository"),
            TryDefaultErrorKind::ParseGitUrl => write!(f, "unable to parse git remote"),
            TryDefaultErrorKind::ListRemotes(_) => write!(f, "unable to list git remotes"),
            TryDefaultErrorKind::AmbiguousGitRemote => write!(f, "unable to choose a git remote to push to, expected to find a remote named 'fork' or 'origin'"),
        }
    }
}

impl Error for TryDefaultError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            TryDefaultErrorKind::RunCommand(err) => Some(err),
            TryDefaultErrorKind::ParseCommandOutput(err) => Some(err),
            TryDefaultErrorKind::OpenRepository(err) => Some(err),
            TryDefaultErrorKind::ParseGitUrl => None,
            TryDefaultErrorKind::ListRemotes(err) => Some(err),
            TryDefaultErrorKind::AmbiguousGitRemote => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum TryDefaultErrorKind {
    #[non_exhaustive]
    RunCommand(io::Error),
    #[non_exhaustive]
    ParseCommandOutput(FromUtf8Error),
    #[non_exhaustive]
    OpenRepository(git2_repository::TryFromPathError),
    #[non_exhaustive]
    ParseGitUrl,
    #[non_exhaustive]
    ListRemotes(git2::Error),
    #[non_exhaustive]
    AmbiguousGitRemote,
}

impl From<TryDefaultErrorKind> for TryDefaultError {
    fn from(kind: TryDefaultErrorKind) -> Self {
        Self { kind }
    }
}

impl From<git2_repository::TryFromPathError> for TryDefaultError {
    fn from(err: git2_repository::TryFromPathError) -> Self {
        Self {
            kind: TryDefaultErrorKind::OpenRepository(err),
        }
    }
}

impl GithubRepositoryMetadata {
    pub fn try_default() -> Result<Self, TryDefaultError> {
        let repo_root = get_repository_root()?;
        let repository = repo_root.as_path().try_into()?;
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

fn get_user_remote(repo: &Repository) -> Result<String, TryDefaultErrorKind> {
    let repo_remotes = repo.remotes().map_err(TryDefaultErrorKind::ListRemotes)?;
    let mut remotes: HashSet<&str> = repo_remotes.iter().flatten().collect();

    remotes
        .take("fork")
        .or_else(|| remotes.take("origin"))
        .map(|str| str.to_owned())
        .ok_or(TryDefaultErrorKind::AmbiguousGitRemote)
}

fn get_repository_root() -> Result<PathBuf, TryDefaultErrorKind> {
    let output_buffer = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .map_err(TryDefaultErrorKind::RunCommand)?
        .stdout;
    let output = String::from_utf8(output_buffer)
        .map_err(TryDefaultErrorKind::ParseCommandOutput)?
        .trim()
        .to_owned();
    Ok(PathBuf::from(output))
}

fn get_remote_url(remote: &str) -> Result<GitUrl, TryDefaultErrorKind> {
    let output_buffer = Command::new("git")
        .arg("config")
        .arg("--get")
        .arg(format!("remote.{remote}.url"))
        .output()
        .map_err(TryDefaultErrorKind::RunCommand)?
        .stdout;
    let output = String::from_utf8(output_buffer)
        .map_err(TryDefaultErrorKind::ParseCommandOutput)?
        .trim()
        .to_owned();
    GitUrl::parse(&output).map_err(|_| TryDefaultErrorKind::ParseGitUrl)
}
