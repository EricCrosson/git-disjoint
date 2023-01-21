#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, ensure, Result};
use git2::{Commit, Repository, RepositoryState, Tree};
use indexmap::IndexMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
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

struct CommitWork<'repo> {
    commit: Commit<'repo>,
    progress_bar: ProgressBar,
}

struct WorkOrder<'repo> {
    issue_group: IssueGroup,
    commit_work: Vec<CommitWork<'repo>>,
    progress_bar: ProgressBar,
}

impl<'repo> WorkOrder<'repo> {
    fn smoosh(&mut self, mut commits: Vec<CommitWork<'repo>>) {
        self.commit_work.append(&mut commits);
    }
}

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
        IssueGroup::Issue(issue_group) => format!(
            "{}-{}",
            issue_group.issue_identifier(),
            summary.to_lowercase()
        ),
        IssueGroup::Commit(summary) => summary.0.clone().to_lowercase(),
    };
    let branch_name = sanitize_git_ref_onelevel(&raw_branch_name).replace(['(', ')'], "-");
    RE_MULTIPLE_HYPHENS
        .replace_all(&branch_name, "-")
        .replace(['\'', '"'], "")
}

#[derive(Debug, Eq, PartialEq)]
enum RedirectOutput {
    DevNull,
    None,
}

fn execute(command: &[&str], redirect_output: RedirectOutput) -> Result<()> {
    let mut runner = Command::new(command[0]);

    if redirect_output == RedirectOutput::DevNull {
        runner.stdout(Stdio::null());
    }

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
        all,
        base,
        choose,
        dry_run,
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
    let processed_commits_by_issue: Vec<WorkOrder> = commits_by_issue
        .into_iter()
        .filter_map(|(issue_group, commits)| {
            let num_commits: u64 = commits.len().try_into().unwrap();
            if let Some(whitelist) = &selected_issues {
                return match whitelist.contains(&issue_group) {
                    true => Some(WorkOrder {
                        issue_group,
                        commit_work: commits
                            .into_iter()
                            .map(|commit| CommitWork {
                                commit,
                                progress_bar: ProgressBar::new(1),
                            })
                            .collect(),
                        progress_bar: ProgressBar::new(num_commits),
                    }),
                    false => None,
                };
            }
            // If there is no whitelist, then operate on every issue
            Some(WorkOrder {
                issue_group,
                commit_work: commits
                    .into_iter()
                    .map(|commit| CommitWork {
                        commit,
                        progress_bar: ProgressBar::new(1),
                    })
                    .collect(),
                progress_bar: ProgressBar::new(num_commits),
            })
        })
        .collect();

    // If `overlay` is active, smoosh all the issue groups into one.
    let processed_commits_by_issue: Vec<WorkOrder> = if overlay {
        processed_commits_by_issue
            .into_iter()
            .fold(Vec::new(), |mut accumulator, work_order| {
                if accumulator.is_empty() {
                    accumulator.push(work_order);
                } else {
                    accumulator
                        .get_mut(0)
                        .unwrap()
                        .smoosh(work_order.commit_work);
                }
                accumulator
            })
    } else {
        processed_commits_by_issue
    };

    // Short-circuit early if there is no work to do.
    if processed_commits_by_issue.is_empty() {
        return Ok(());
    }

    let style_issue_group_pending = ProgressStyle::with_template("  {msg}").unwrap();
    let style_issue_group_working = ProgressStyle::with_template("> {msg}").unwrap();
    let style_issue_group_done = ProgressStyle::with_template("✔ {msg}").unwrap();
    let style_commit_pending = ProgressStyle::with_template("    {msg}").unwrap();
    let style_commit_working = ProgressStyle::with_template("  > {msg}").unwrap();
    let style_commit_done = ProgressStyle::with_template("  ✔ {msg}").unwrap();

    let multi_progress_bar = MultiProgress::new();

    for work_order in processed_commits_by_issue.iter() {
        // Insert one progress bar for the issue group
        work_order
            .progress_bar
            .set_style(style_issue_group_pending.clone());
        multi_progress_bar.insert_from_back(0, work_order.progress_bar.clone());
        work_order
            .progress_bar
            .set_message(format!("{}", work_order.issue_group));

        // and one progress bar for each ticket
        for commit_work in work_order.commit_work.iter() {
            commit_work
                .progress_bar
                .set_style(style_commit_pending.clone());
            multi_progress_bar.insert_from_back(0, commit_work.progress_bar.clone());
            commit_work
                .progress_bar
                .set_message(format!("{}", commit_work.commit.summary().unwrap()));
        }
    }

    processed_commits_by_issue
        .into_iter()
        .filter(|work_order| {
            if let Some(whitelist) = &selected_issues {
                return whitelist.contains(&work_order.issue_group);
            }
            // If there is no whitelist, then operate on every issue
            true
        })
        .try_for_each(|work_order| -> Result<()> {
            work_order
                .progress_bar
                .set_style(style_issue_group_working.clone());
            work_order.progress_bar.tick();

            // Grab the first summary to convert into a branch name.
            // We only choose the first summary because we know each Vec is
            // non-empty and the first element is convenient.
            let summary = {
                let commit = &work_order.commit_work[0].commit;
                commit.summary().ok_or_else(|| {
                    anyhow!(
                        "Summary for commit {:?} is not a valid UTF-8 string",
                        commit.id()
                    )
                })?
            };

            let branch_name = get_branch_name(&work_order.issue_group, summary);
            let branch_ref = format!("refs/heads/{}", branch_name);
            let branch_obj = repo.revparse_single(&branch_ref);

            // If branch already exists, assume we've already handled this ticket
            // DISCUSS: in the future, we could compare this branch against the desired
            // commits, and add any missing commits to this branch and then update the remote
            if branch_obj.is_ok() {
                eprintln!("Warning: a branch named {:?} already exists", branch_name);
                return Ok(());
            }

            if !dry_run {
                // Create a branch
                repo.branch(&branch_name, start_point_commit, true)?;

                // Check out the new branch
                let branch_obj = repo.revparse_single(&branch_ref)?;
                repo.checkout_tree(&branch_obj, None)?;
                repo.set_head(&branch_ref)?;
            }

            // Cherry-pick commits related to the target issue
            for commit_work in work_order.commit_work.iter() {
                commit_work
                    .progress_bar
                    .set_style(style_commit_working.clone());

                if dry_run {
                    for _ in 1..50 {
                        std::thread::sleep(std::time::Duration::from_millis(15));
                        commit_work.progress_bar.tick();
                    }
                } else {
                    execute(
                        &[
                            "git",
                            "cherry-pick",
                            "--allow-empty",
                            &commit_work.commit.id().to_string(),
                        ],
                        RedirectOutput::DevNull,
                    )?;
                }

                commit_work
                    .progress_bar
                    .set_style(style_commit_done.clone());
                commit_work.progress_bar.finish()
            }

            if !dry_run {
                // Push the branch
                execute(
                    &["git", "push", &user_config.remote, &branch_name],
                    RedirectOutput::DevNull,
                )?;

                // Open a pull request
                // Only ask the user to edit the PR metadata when multiple commits
                // create ambiguity about the contents of the PR title and body.
                let edit = work_order.commit_work.len() > 1;
                execute(
                    &[
                        "hub",
                        "pull-request",
                        "--browse",
                        "--draft",
                        if edit { "--edit" } else { "--no-edit" },
                    ],
                    RedirectOutput::None,
                )?;

                // Finally, check out the original ref
                execute(&["git", "checkout", "-"], RedirectOutput::DevNull)?;
            }

            work_order
                .progress_bar
                .set_style(style_issue_group_done.clone());
            work_order.progress_bar.finish();

            Ok(())
        })?;

    Ok(())
}
