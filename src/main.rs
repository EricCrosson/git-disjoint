#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, ensure, Result};
use git2::{Commit, Repository, RepositoryState, Tree};
use indexmap::IndexMap;
use lazy_static::lazy_static;
use regex::Regex;
use sanitize_git_ref::sanitize_git_ref_onelevel;

mod args;
mod default_branch;
mod interact;
mod issue;
mod issue_group;
mod sanitized_args;
mod user_config;

use crate::issue::Issue;
use crate::issue_group::IssueGroup;
use crate::sanitized_args::SanitizedArgs;
use crate::user_config::{get_user_remote, UserConfig};

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

/// Return the list of commits from `base` to `HEAD`, sorted parent-first,
/// children-last.
fn get_commits_from_base<'repo>(
    repo: &'repo Repository,
    base: &git2::Object,
) -> Result<Vec<Commit<'repo>>> {
    // Identifies output commits by traversing commits starting from HEAD and
    // working towards base, then reversing the list.
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

    let mut commits: Vec<Commit> = revwalk
        .filter_map(|id| {
            let id = filter_try!(id);
            let commit = filter_try!(repo.find_commit(id));
            Some(commit)
        })
        // Only include commits after the `start_point`
        .take_while(|commit| !base.id().eq(&commit.id()))
        .collect();

    // Order commits parent-first, children-last
    commits.reverse();

    Ok(commits)
}

/// Create a valid git branch name, by:
/// - concatenating the issue and summary, separated by a hyphen
/// - replace parenthesis with hyphens
///   since parenthesis interfere with terminal tab-completion,
/// - delete single and double quotes
///   since quotes interfere with terminal tab-completion,
/// - lower-case all letters in the commit message summary (but not the ticket name)
fn get_branch_name(issue: &IssueGroup, summary: &str) -> String {
    let raw_branch_name = match issue {
        IssueGroup::Issue(issue) => format!("{}-{}", issue, summary.to_lowercase()),
        IssueGroup::Commit(summary) => summary.0.clone().to_lowercase(),
    };
    let branch_name = sanitize_git_ref_onelevel(&raw_branch_name).replace(['(', ')'], "-");
    RE_MULTIPLE_HYPHENS
        .replace_all(&branch_name, "-")
        .replace(['\'', '"'], "")
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

fn get_repository_root() -> Result<PathBuf> {
    let output_buffer = Command::new("git")
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()?
        .stdout;
    let output = String::from_utf8(output_buffer)?.trim().to_owned();
    Ok(PathBuf::from(output))
}

fn main() -> Result<()> {
    // DISCUSS: moving the `repo` into SanitizedArgs
    let SanitizedArgs {
        base,
        choose,
        all,
        overlay,
        separate,
    } = SanitizedArgs::parse()?;

    let root = get_repository_root()?;
    let repo = Repository::open(root)?;

    let originally_checked_out_commit = repo.head()?.resolve()?.peel_to_commit()?;
    let originally_checked_out_tree = originally_checked_out_commit.tree()?;

    // Assume `base` indicates a single commit
    let start_point = repo.revparse_single(&base.0)?;
    let start_point_commit = start_point
        .as_commit()
        .ok_or_else(|| anyhow!("Expected `--base` to identify a commit"))?;

    assert_repository_state_is_clean(&repo)?;
    assert_tree_matches_workdir_with_index(&repo, &originally_checked_out_tree)?;

    let commits = get_commits_from_base(&repo, &start_point)?;

    let user_config = UserConfig {
        remote: get_user_remote(&repo)?,
    };

    let commits_by_issue: IndexMap<IssueGroup, Vec<Commit>> = commits
        .into_iter()
        // Parse issue from commit message
        .map(|commit| -> Result<Option<(IssueGroup, Commit)>> {
            let issue = commit.message().and_then(Issue::parse_from_commit_message);
            // If:
            // - we're not treating every commit separately, and
            // - this commit includes an issue,
            // then add this commit to that issue's group.
            if !separate {
                if let Some(issue) = issue {
                    return Ok(Some((IssueGroup::Issue(issue), commit)));
                }
            }

            // If:
            // - the user requested we treat every issue separately, or
            // - this commit does not include an issue, but the user has
            //   --specified the all flag, then
            // add this commit to a unique issue-group.
            if separate || all {
                return Ok(Some((IssueGroup::Commit((&commit).try_into()?), commit)));
            }

            // Otherwise, skip this commit.
            eprintln!(
                "Warning: ignoring commit without issue footer: {:?}",
                commit.id()
            );
            Ok(None)
        })
        // unwrap the Result
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        // drop the None values
        .flatten()
        .fold(IndexMap::new(), |mut map, (issue, commit)| {
            let commits = map.entry(issue).or_default();
            commits.push(commit);
            map
        });

    let selected_issues = if choose || overlay {
        let keys = commits_by_issue.keys().collect();
        Some(
            interact::select_issues(keys)
                .map_err(|_| anyhow!("Unable to process issue selection"))?,
        )
    } else {
        None
    };

    // Construct this map assuming `overlay` is not active:
    // Each issue group gets its own branch and PR.
    let processed_commits_by_issue: Vec<(IssueGroup, Vec<Commit>)> = commits_by_issue
        .into_iter()
        .filter(|(issue, _commit)| {
            if let Some(whitelist) = &selected_issues {
                return whitelist.contains(issue);
            }
            // If there is no whitelist, then operate on every issue
            true
        })
        .collect();

    // If `overlay` is active, smoosh all the issue groups into one.
    let processed_commits_by_issue: Vec<(IssueGroup, Vec<Commit>)> = if overlay {
        processed_commits_by_issue.into_iter().fold(
            Vec::new(),
            |mut accumulator, (issue_group, mut commits)| {
                if accumulator.is_empty() {
                    accumulator.push((issue_group, commits));
                } else {
                    accumulator.get_mut(0).unwrap().1.append(&mut commits);
                }
                accumulator
            },
        )
    } else {
        processed_commits_by_issue
    };

    processed_commits_by_issue
        .into_iter()
        .filter(|(issue, _commit)| {
            if let Some(whitelist) = &selected_issues {
                return whitelist.contains(issue);
            }
            // If there is no whitelist, then operate on every issue
            true
        })
        .try_for_each(|(issue, commits)| -> Result<()> {
            // DEBUG:
            println!("{}: {:#?}", issue, commits);

            // Grab the first summary to convert into a branch name.
            // We only choose the first summary because we know each Vec is
            // non-empty and the first element is convenient.
            let summary = {
                let commit = &commits[0];
                commits[0].summary().ok_or_else(|| {
                    anyhow!(
                        "Summary for commit {:?} is not a valid UTF-8 string",
                        commit.id()
                    )
                })?
            };

            let branch_name = get_branch_name(&issue, summary);
            let branch_ref = format!("refs/heads/{}", branch_name);
            let branch_obj = repo.revparse_single(&branch_ref);

            // If branch already exists, assume we've already handled this ticket
            // DISCUSS: in the future, we could compare this branch against the desired
            // commits, and add any missing commits to this branch and then update the remote
            if branch_obj.is_ok() {
                eprintln!("Warning: a branch named {:?} already exists", branch_name);
                return Ok(());
            }

            // Create a branch
            repo.branch(&branch_name, start_point_commit, true)?;

            // Check out the new branch
            let branch_obj = repo.revparse_single(&branch_ref)?;
            repo.checkout_tree(&branch_obj, None)?;
            repo.set_head(&branch_ref)?;

            // Cherry-pick commits related to the target issue
            for commit in commits.iter() {
                // DEBUG:
                println!("Cherry-picking commit {}", &commit.id());
                execute(&[
                    "git",
                    "cherry-pick",
                    "--allow-empty",
                    &commit.id().to_string(),
                ])?;
            }

            // Push the branch
            execute(&["git", "push", &user_config.remote, &branch_name])?;

            // Open a pull request
            // Only ask the user to edit the PR metadata when multiple commits
            // create ambiguity about the contents of the PR title and body.
            let edit = commits.len() > 1;
            execute(&[
                "hub",
                "pull-request",
                "--browse",
                "--draft",
                if edit { "--edit" } else { "--no-edit" },
            ])?;

            // Finally, check out the original ref
            execute(&["git", "checkout", "-"])?;

            Ok(())
        })?;

    Ok(())
}
