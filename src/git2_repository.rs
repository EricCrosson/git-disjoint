use std::ops::Deref;

use git2::Commit;

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

impl Repository {
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
