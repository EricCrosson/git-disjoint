#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::env::temp_dir;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, ensure, Result};
use default_branch::DefaultBranch;
use git2::{Commit, Repository, RepositoryState};
use indexmap::IndexMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use interact::{select_issues, IssueGroupWhitelist};
use lazy_static::lazy_static;
use regex::Regex;
use sanitize_git_ref::sanitize_git_ref_onelevel;
use sanitized_args::{CommitGrouping, CommitsToConsider, OverlayCommitsIntoOnePullRequest};
use serde::{Deserialize, Serialize};

mod args;
mod default_branch;
mod editor;
mod interact;
mod issue;
mod issue_group;
mod sanitized_args;

use crate::editor::{interactive_get_pr_metadata, PullRequestMetadata};
use crate::issue::Issue;
use crate::issue_group::IssueGroup;
use crate::sanitized_args::SanitizedArgs;

// git2 resources:
// - https://siciarz.net/24-days-rust-git2/

// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.

const PREFIX_PENDING: &'static str = " ";
const PREFIX_WORKING: &'static str = ">";
const PREFIX_DONE: &'static str = "✔";

lazy_static! {
    static ref RE_MULTIPLE_HYPHENS: Regex =
        Regex::new("-{2,}").expect("Expected multiple-hyphens regular expression to compile");
    static ref STYLE_ISSUE_GROUP: ProgressStyle =
        ProgressStyle::with_template("{prefix} {msg}").unwrap();
    static ref STYLE_COMMIT: ProgressStyle =
        ProgressStyle::with_template("  {prefix} {msg}").unwrap();
}

#[derive(Debug)]
struct CommitWork<'repo> {
    commit: Commit<'repo>,
    progress_bar: ProgressBar,
}

impl<'repo> From<Commit<'repo>> for CommitWork<'repo> {
    fn from(commit: Commit<'repo>) -> Self {
        let progress_bar = ProgressBar::new(1);
        progress_bar.set_style(STYLE_COMMIT.clone());
        progress_bar.set_prefix(PREFIX_PENDING);
        progress_bar.set_message(format!("{}", commit.summary().unwrap()));
        Self {
            commit,
            progress_bar,
        }
    }
}

#[derive(Debug)]
struct WorkOrder<'repo> {
    issue_group: IssueGroup,
    commit_work: Vec<CommitWork<'repo>>,
    progress_bar: ProgressBar,
}

impl<'repo> From<(IssueGroup, Vec<Commit<'repo>>)> for WorkOrder<'repo> {
    fn from((issue_group, commits): (IssueGroup, Vec<Commit<'repo>>)) -> Self {
        let num_commits: u64 = commits.len().try_into().unwrap();
        let progress_bar = ProgressBar::new(num_commits);
        progress_bar.set_style(STYLE_ISSUE_GROUP.clone());
        progress_bar.set_prefix(PREFIX_PENDING);
        progress_bar.set_message(format!("{}", issue_group));
        WorkOrder {
            issue_group,
            commit_work: commits.into_iter().map(CommitWork::from).collect(),
            progress_bar,
        }
    }
}

// https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#create-a-pull-request
#[derive(Debug, Serialize)]
struct CreatePullRequestRequest {
    title: String,
    body: String,
    head: String,
    base: String,
    draft: bool,
}

// https://docs.github.com/en/rest/pulls/pulls?apiVersion=2022-11-28#create-a-pull-request
#[derive(Debug, Deserialize)]
struct CreatePullRequestResponse {
    url: String,
}

macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(_) => return None,
        }
    };
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
        runner.stderr(Stdio::null());
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
fn assert_tree_matches_workdir_with_index(repo: &Repository) -> Result<()> {
    let originally_checked_out_commit = repo.head()?.resolve()?.peel_to_commit()?;
    let originally_checked_out_tree = originally_checked_out_commit.tree()?;

    let files_changed = repo
        .diff_tree_to_workdir_with_index(Some(&originally_checked_out_tree), None)?
        .stats()?
        .files_changed();
    ensure!(
        files_changed == 0,
        "Repository should not contain staged or unstaged changes to tracked files"
    );
    Ok(())
}

fn get_log_file() -> PathBuf {
    let start = SystemTime::now();
    temp_dir().join(format!(
        "git-disjoint-{:?}",
        start.duration_since(UNIX_EPOCH).unwrap()
    ))
}

fn get_base_commit<'repo>(repo: &'repo Repository, base: &DefaultBranch) -> Result<Commit<'repo>> {
    // Assumption: `base` indicates a single commit
    // Assumption: `origin` is the upstream/main repositiory
    let start_point = repo.revparse_single(&format!("origin/{}", &base.0))?;
    start_point
        .as_commit()
        .ok_or_else(|| anyhow!("Expected `--base` to identify a commit"))
        .cloned()
}

/// Return the list of commits from `base` to `HEAD`, sorted parent-first,
/// children-last.
fn get_commits_since_base<'repo>(
    repo: &'repo Repository,
    base: &git2::Commit,
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

fn group_commits_by_issue_group<'repo>(
    commits: Vec<Commit<'repo>>,
    commits_to_consider: CommitsToConsider,
    commit_grouping: CommitGrouping,
) -> Result<IndexMap<IssueGroup, Vec<Commit>>> {
    let commits_by_issue: IndexMap<IssueGroup, Vec<Commit>> = commits
        .into_iter()
        // Parse issue from commit message
        .map(|commit| -> Result<Option<(IssueGroup, Commit)>> {
            let issue = commit.message().and_then(Issue::parse_from_commit_message);
            // If:
            // - we're grouping commits by issue, and
            // - this commit includes an issue,
            // then add this commit to that issue's group.
            if commit_grouping == CommitGrouping::ByIssue {
                if let Some(issue) = issue {
                    return Ok(Some((IssueGroup::Issue(issue), commit)));
                }
            }

            // If:
            // - we're treating every issue separately, or
            // - we're considering all commits (even commits without an issue),
            // add this commit to a unique issue-group.
            if commit_grouping == CommitGrouping::Individual
                || commits_to_consider == CommitsToConsider::All
            {
                return Ok(Some((IssueGroup::Commit((&commit).try_into()?), commit)));
            }

            // Otherwise, skip this commit.
            eprintln!(
                "Warning: ignoring commit without issue trailer: {:?}",
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

    Ok(commits_by_issue)
}

fn filter_issue_groups_by_whitelist<'repo>(
    commits_by_issue_group: IndexMap<IssueGroup, Vec<Commit<'repo>>>,
    selected_issue_groups: &IssueGroupWhitelist,
) -> IndexMap<IssueGroup, Vec<Commit<'repo>>> {
    match &selected_issue_groups {
        // If there is a whitelist, only operate on issue_groups in the whitelist
        IssueGroupWhitelist::Whitelist(whitelist) => commits_by_issue_group
            .into_iter()
            .filter(|(issue_group, _commits)| whitelist.contains(&issue_group))
            .collect(),
        // If there is no whitelist, then operate on every issue
        IssueGroupWhitelist::WhitelistDNE => commits_by_issue_group,
    }
}

fn apply_overlay<'repo>(
    commits_by_issue_group: IndexMap<IssueGroup, Vec<Commit<'repo>>>,
    overlay: OverlayCommitsIntoOnePullRequest,
) -> IndexMap<IssueGroup, Vec<Commit<'repo>>> {
    match overlay {
        // If we are overlaying all active issue groups into one PR,
        // combine all active commits under the first issue group
        OverlayCommitsIntoOnePullRequest::Yes => commits_by_issue_group
            .into_iter()
            .reduce(|mut accumulator, mut item| {
                accumulator.1.append(&mut item.1);
                accumulator
            })
            // Map the option back into an IndexMap
            .map(|(issue_group, commits)| {
                let mut map = IndexMap::with_capacity(1);
                map.insert(issue_group, commits);
                map
            })
            .unwrap_or_default(),
        // If we are not overlaying issue groups, keep them separate
        OverlayCommitsIntoOnePullRequest::No => commits_by_issue_group,
    }
}

fn do_git_disjoint(sanitized_args: SanitizedArgs, log_file: PathBuf) -> Result<()> {
    let SanitizedArgs {
        all,
        base,
        choose,
        dry_run,
        github_token,
        overlay,
        separate,
        repository,
        repository_metadata,
    } = sanitized_args;

    let base_commit = get_base_commit(&repository, &base)?;
    let commits = get_commits_since_base(&repository, &base_commit)?;
    // We have to make a first pass to determine the issue groups in play
    let commits_by_issue_group = group_commits_by_issue_group(commits, all, separate)?;
    let selected_issue_groups = select_issues(&commits_by_issue_group, choose, overlay)?;
    // Now filter the set of all issue groups to just the whitelisted issue groups
    let commits_by_issue_group =
        filter_issue_groups_by_whitelist(commits_by_issue_group, &selected_issue_groups);
    let commits_by_issue_group = apply_overlay(commits_by_issue_group, overlay);

    let work_orders: Vec<WorkOrder> = commits_by_issue_group
        .into_iter()
        .map(WorkOrder::from)
        .collect();

    // Short-circuit early if there is no work to do.
    if work_orders.is_empty() {
        return Ok(());
    }

    let http_client = reqwest::blocking::Client::new();

    let multi_progress_bar = MultiProgress::new();

    for work_order in work_orders.iter() {
        // Insert one progress bar for the issue group
        multi_progress_bar.insert_from_back(0, work_order.progress_bar.clone());
        // and one progress bar for each ticket
        for commit_work in work_order.commit_work.iter() {
            multi_progress_bar.insert_from_back(0, commit_work.progress_bar.clone());
        }
    }

    work_orders
        .into_iter()
        .try_for_each(|work_order| -> Result<()> {
            work_order.progress_bar.set_prefix(PREFIX_WORKING);
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
            let branch_obj = repository.revparse_single(&branch_ref);

            // If branch already exists, assume we've already handled this ticket
            // DISCUSS: in the future, we could compare this branch against the desired
            // commits, and add any missing commits to this branch and then update the remote
            if branch_obj.is_ok() {
                eprintln!("Warning: a branch named {:?} already exists", branch_name);
                return Ok(());
            }

            if !dry_run {
                // Create a branch
                repository.branch(&branch_name, &base_commit, true)?;

                // Check out the new branch
                let branch_obj = repository.revparse_single(&branch_ref)?;
                repository.checkout_tree(&branch_obj, None)?;
                repository.set_head(&branch_ref)?;
            }

            // Cherry-pick commits related to the target issue
            for commit_work in work_order.commit_work.iter() {
                commit_work.progress_bar.set_prefix(PREFIX_WORKING);

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

                commit_work.progress_bar.set_prefix(PREFIX_DONE);
                commit_work.progress_bar.finish()
            }

            if !dry_run {
                // Push the branch
                execute(
                    &["git", "push", &repository_metadata.remote, &branch_name],
                    RedirectOutput::DevNull,
                )?;

                // Open a pull request
                // Only ask the user to edit the PR metadata when multiple commits
                // create ambiguity about the contents of the PR title and body.
                let needs_edit = work_order.commit_work.len() > 1;

                let pr_metadata = match needs_edit {
                    true => interactive_get_pr_metadata(
                        &repository_metadata.root,
                        work_order
                            .commit_work
                            .iter()
                            .map(|commit_work| &commit_work.commit)
                            .collect(),
                    )?,
                    false => {
                        let commit = &work_order.commit_work.get(0).unwrap().commit;
                        PullRequestMetadata {
                            title: commit.summary().unwrap().to_owned(),
                            body: commit.body().unwrap().to_owned(),
                        }
                    }
                };

                let response: CreatePullRequestResponse = http_client
                    .post(format!(
                        "https://api.github.com/repos/{}/{}/pulls",
                        repository_metadata.owner, repository_metadata.name
                    ))
                    .header("User-Agent", "git-disjoint")
                    .header("Accept", "application/vnd.github.v3+json")
                    .header("Authorization", format!("token {}", github_token))
                    .json(&CreatePullRequestRequest {
                        title: pr_metadata.title.clone(),
                        body: pr_metadata.body.clone(),
                        head: format!("{}:{}", repository_metadata.forker, branch_name),
                        base: base.0.clone(),
                        draft: true,
                    })
                    .send()
                    .map_err(|request_error| {
                        anyhow!("Error contacting the GitHub API: {request_error}")
                    })?
                    .json()
                    .map_err(|response_error| {
                        anyhow!("Error parsing the GitHub API response: {response_error}")
                    })?;

                open::that(response.url)?;

                // Finally, check out the original ref
                execute(&["git", "checkout", "-"], RedirectOutput::DevNull)?;
            }

            work_order.progress_bar.set_prefix(PREFIX_DONE);
            work_order.progress_bar.finish();

            Ok(())
        })?;

    Ok(())
}

fn main() -> Result<()> {
    let sanitized_args = SanitizedArgs::parse()?;
    let log_file = get_log_file();

    assert_repository_state_is_clean(&sanitized_args.repository)?;
    assert_tree_matches_workdir_with_index(&sanitized_args.repository)?;

    // TODO: rename for clarity
    do_git_disjoint(sanitized_args, log_file)
}
