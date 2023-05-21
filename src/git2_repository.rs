use std::{
    error::Error,
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
};

use git2::{Commit, RepositoryState};

use crate::default_branch::DefaultBranch;

pub(crate) struct Repository(git2::Repository);

impl Deref for Repository {
    type Target = git2::Repository;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<git2::Repository> for Repository {
    fn from(value: git2::Repository) -> Self {
        Self(value)
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct FromPathError {
    path: PathBuf,
    kind: FromPathErrorKind,
}

impl Display for FromPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            FromPathErrorKind::OpenRepository(_) => {
                write!(f, "unable to open git repository in {:?}", self.path)
            }
            FromPathErrorKind::OperationInProgress(state) => write!(
                f,
                "expected repository to be in a clean state, got {state:?}"
            ),
            FromPathErrorKind::UncommittedFiles => write!(
                f,
                "repository contains staged or unstaged changes to tracked files"
            ),
            FromPathErrorKind::Git(_) => write!(f, "git error"),
        }
    }
}

impl Error for FromPathError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            FromPathErrorKind::OpenRepository(err) => Some(err),
            FromPathErrorKind::OperationInProgress(_) => None,
            FromPathErrorKind::UncommittedFiles => None,
            FromPathErrorKind::Git(err) => Some(err),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FromPathErrorKind {
    #[non_exhaustive]
    OpenRepository(git2::Error),
    #[non_exhaustive]
    OperationInProgress(RepositoryState),
    #[non_exhaustive]
    UncommittedFiles,
    #[non_exhaustive]
    Git(git2::Error),
}

impl TryFrom<&Path> for Repository {
    type Error = FromPathError;

    fn try_from(root: &Path) -> Result<Self, Self::Error> {
        (|| {
            let repo: Repository = git2::Repository::open(root)
                .map(Into::into)
                .map_err(FromPathErrorKind::OpenRepository)?;

            repo.assert_repository_state_is_clean()?;
            repo.assert_tree_matches_workdir_with_index()?;
            Ok(repo)
        })()
        .map_err(|kind| FromPathError {
            path: root.to_owned(),
            kind,
        })
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub(crate) struct BaseCommitError {
    base: DefaultBranch,
    kind: BaseCommitErrorKind,
}

impl Display for BaseCommitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            BaseCommitErrorKind::Revparse(_) => write!(f, "git rev-parse error"),
            BaseCommitErrorKind::AmbigiousBase => {
                write!(f, "expected --base to identify a ref, got {}", self.base)
            }
        }
    }
}

impl Error for BaseCommitError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            BaseCommitErrorKind::Revparse(err) => Some(err),
            BaseCommitErrorKind::AmbigiousBase => None,
        }
    }
}

#[derive(Debug)]
pub(crate) enum BaseCommitErrorKind {
    #[non_exhaustive]
    Revparse(git2::Error),
    #[non_exhaustive]
    AmbigiousBase,
}

impl Repository {
    /// Return an error if the repository state is not clean.
    ///
    /// This prevents invoking `git disjoint` on a repository in the middle
    /// of some other operation, like a `git rebase`.
    fn assert_repository_state_is_clean(&self) -> Result<(), FromPathErrorKind> {
        let state = self.state();
        match state {
            RepositoryState::Clean => Ok(()),
            _ => Err(FromPathErrorKind::OperationInProgress(state)),
        }
    }

    /// Return an error if there are any diffs to tracked files, staged or unstaged.
    ///
    /// This emulates `git diff` by diffing the tree to the index and the index to
    /// the working directory and blending the results into a single diff that includes
    /// staged, deletec, etc.
    ///
    /// This check currently excludes untracked files, but I'm not tied to this behavior.
    fn assert_tree_matches_workdir_with_index(&self) -> Result<(), FromPathErrorKind> {
        let files_changed = (|| {
            let originally_checked_out_commit = self.head()?.resolve()?.peel_to_commit()?;
            let originally_checked_out_tree = originally_checked_out_commit.tree()?;

            let files_changed = self
                .diff_tree_to_workdir_with_index(Some(&originally_checked_out_tree), None)?
                .stats()?
                .files_changed();
            Ok(files_changed)
        })()
        .map_err(FromPathErrorKind::Git)?;

        match files_changed {
            0 => Ok(()),
            _ => Err(FromPathErrorKind::UncommittedFiles),
        }
    }

    /// Assumption: `base` indicates a single commit
    /// Assumption: `origin` is the upstream/main repositiory
    pub fn base_commit(&self, base: &DefaultBranch) -> Result<Commit, BaseCommitError> {
        (|| {
            let start_point = self
                .revparse_single(&format!("origin/{}", &base.0))
                .map_err(BaseCommitErrorKind::Revparse)?;
            start_point
                .as_commit()
                .ok_or(BaseCommitErrorKind::AmbigiousBase)
                .cloned()
        })()
        .map_err(|kind| BaseCommitError {
            base: base.to_owned(),
            kind,
        })
    }

    /// Return the list of commits from `base` to `HEAD`, sorted parent-first,
    /// children-last.
    pub fn commits_since_base(&self, base: &Commit) -> Result<Vec<Commit>, anyhow::Error> {
        macro_rules! filter_try {
            ($e:expr) => {
                match $e {
                    Ok(t) => t,
                    Err(_) => return None,
                }
            };
        }

        // Identifies output commits by traversing commits starting from HEAD and
        // working towards base, then reversing the list.
        let mut revwalk = self.revwalk()?;
        revwalk.push_head()?;

        revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

        let mut commits: Vec<Commit> = revwalk
            .filter_map(|id| {
                let id = filter_try!(id);
                let commit = filter_try!(self.find_commit(id));
                Some(commit)
            })
            // Only include commits after the `start_point`
            .take_while(|commit| !base.id().eq(&commit.id()))
            .collect();

        // commits are now ordered child-first, parent-last

        // Order commits parent-first, children-last
        commits.reverse();

        Ok(commits)
    }
}
