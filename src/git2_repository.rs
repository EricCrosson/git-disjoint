use std::{error::Error, fmt::Display, ops::Deref};

use git2::Commit;

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
    // RESUME: avoid anyhow
    /// Assumption: `base` indicates a single commit
    /// Assumption: `origin` is the upstream/main repositiory
    pub fn base_commit(&self, base: &DefaultBranch) -> Result<Commit, BaseCommitError> {
        Ok((|| {
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
        })?)
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
