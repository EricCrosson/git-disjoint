#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::collections::HashMap;
use std::process::Command;

use anyhow::{anyhow, ensure, Result};
use clap::Parser;
use default_branch::DefaultBranch;
use git2::{Commit, Repository, RepositoryState, Tree};
use lazy_static::lazy_static;
use regex::Regex;
use sanitize_git_ref::sanitize_git_ref_onelevel;

mod args;
mod default_branch;
mod issue;

use crate::args::Args;
use crate::issue::Issue;

// git2 resources:
// - https://siciarz.net/24-days-rust-git2/

// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.

lazy_static! {
    static ref RE_MULTIPLE_HYPHENS: Regex =
        Regex::new("-{2,}").expect("Expected multiple-hyphens regular expression to compile");
}

macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(_) => return None,
        }
    };
}

fn get_branch_name(issue: &Issue, summary: &str) -> String {
    // Replace parentheses, because they interfere with terminal tab-completion
    // (they require double quotes).
    let branch_name = sanitize_git_ref_onelevel(&format!("{}-{}", issue, summary))
        .replace('(', "-")
        .replace(')', "-");
    RE_MULTIPLE_HYPHENS
        .replace_all(&branch_name, "-")
        .to_string()
}

fn execute(command: &[&str]) -> Result<()> {
    let mut runner = Command::new(command[0]);
    for argument in command.iter().skip(1) {
        runner.arg(argument);
    }

    // Try to run the command
    let status = runner.status()?;

    // Return an Err if the exit status is non-zero
    if let Err(error) = status.exit_ok() {
        return Err(error)?;
    }
    Ok(())
}

/// Return an error if the repository state is not clean.
///
/// This prevents invoking `git disjoint` on a repository in the middle
/// of some other operation, like a `git rebase`.
fn assert_repository_state_is_clean(repo: &Repository) -> Result<()> {
    let state = repo.state();
    ensure!(
        RepositoryState::Clean == state,
        "Repository should be in a clean state, not {:?}",
        state
    );
    Ok(())
}

/// Return an error if there are any diffs to tracked files, staged or unstaged.
///
/// This emulates `git diff` by diffing the tree to the index and the index to
/// the working directory and blending the results into a single diff that includes
/// staged, deletec, etc.
///
/// This check currently excludes untracked files, but I'm not tied to this behavior.
fn assert_tree_matches_workdir_with_index(repo: &Repository, old_tree: &Tree) -> Result<()> {
    let files_changed = repo
        .diff_tree_to_workdir_with_index(Some(old_tree), None)?
        .stats()?
        .files_changed();
    ensure!(
        files_changed == 0,
        "Repository should not contain staged or unstaged changes to tracked files"
    );
    Ok(())
}

fn main() -> Result<()> {
    let Args { since } = Args::parse();
    let since = match since {
        Some(since) => since,
        None => DefaultBranch::try_get_default()?,
    };
    let repo = Repository::open(".")?;

    let originally_checked_out_commit = repo.head()?.resolve()?.peel_to_commit()?;
    let originally_checked_out_tree = originally_checked_out_commit.tree()?;

    // Assume `since` indicates a single commit
    let start_point = repo.revparse_single(&since.0)?;
    let start_point_commit = start_point
        .as_commit()
        .ok_or_else(|| anyhow!("Expected `--since` to identify a commit"))?;

    assert_repository_state_is_clean(&repo)?;
    assert_tree_matches_workdir_with_index(&repo, &originally_checked_out_tree)?;

    // Traverse commits starting from HEAD
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

    let commits: Vec<Commit> = {
        let mut commits: Vec<Commit> = revwalk
            .filter_map(|id| {
                let id = filter_try!(id);
                let commit = filter_try!(repo.find_commit(id));
                Some(commit)
            })
            // Only include commits after the `start_point`
            .take_while(|commit| !start_point.id().eq(&commit.id()))
            .collect();

        // Order commits parent-first, children-last
        commits.reverse();
        commits
    };

    let commits_by_issue = commits
        .into_iter()
        // Parse issue from commit message
        .filter_map(|commit| -> Option<(Issue, Commit)> {
            let issue = commit.message().and_then(Issue::parse_from_commit_message);
            if let Some(issue) = issue {
                Some((issue, commit))
            } else {
                eprintln!(
                    "Warning: ignoring commit without issue footer: {:?}",
                    commit.id()
                );
                None
            }
        })
        .fold(
            HashMap::<Issue, Vec<Commit>>::new(),
            |mut map, (issue, commit)| {
                let commits = map.entry(issue).or_default();
                commits.push(commit);
                map
            },
        );

    commits_by_issue
        .into_iter()
        .try_for_each(|(issue, commits)| -> Result<()> {
            // DEBUG:
            println!("{:#?}: {:#?}", issue, commits);

            // Grab the first summary to convert into a branch name.
            // We only choose the first summary because we know each Vec is
            // non-empty and the first element is convenient.
            let summary = {
                let commit = &commits[0];
                commits[0].summary().ok_or_else(|| {
                    anyhow!("Summary for commit {:?} is not valid UTF-8", commit.id())
                })?
            };

            let branch_name = get_branch_name(&issue, summary);
            let branch_ref = format!("refs/heads/{}", &branch_name);
            let branch_obj = repo.revparse_single(&branch_ref);

            // If branch already exists, assume we've already handled this ticket
            // DISCUSS: in the future, we could compare this branch against the desired
            // commits, and add any missing commits to this branch and then update the remote
            if let Ok(branch_obj) = branch_obj {
                eprintln!(
                    "Warning: a branch named {:?} already exists",
                    branch_obj.id().to_string()
                );
                return Ok(());
            }

            // Create a branch
            repo.branch(&branch_name, start_point_commit, true)?;

            // Check out the new branch
            let branch_obj = repo.revparse_single(&branch_ref)?;
            repo.checkout_tree(&branch_obj, None)?;
            repo.set_head(&branch_ref)?;

            // Cherry-pick commits related to the target issue
            for commit in commits {
                // DEBUG:
                println!("Cherry-picking commit {}", &commit.id());
                execute(&["git", "cherry-pick", &commit.id().to_string()])?;
            }

            // Push the branch
            execute(&["git", "push", "origin", &branch_name])?;

            // Open a pull request
            execute(&["hub", "pull-request", "--browse", "--draft"])?;

            // Finally, check out the original ref
            execute(&["git", "checkout", "-"])?;

            Ok(())
        })?;

    Ok(())
}
