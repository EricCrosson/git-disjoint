#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, ensure};
use async_executors::{TokioTp, TokioTpBuilder};
use async_nursery::{NurseExt, Nursery};
use clap::Parser;
use default_branch::DefaultBranch;
use futures::TryStreamExt;
use git2::{Commit, Repository, RepositoryState};
use indexmap::IndexMap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lazy_static::lazy_static;
use log_file::LogFile;
use regex::Regex;
use serde::{Deserialize, Serialize};

mod branch_name;
mod cli;
mod default_branch;
mod editor;
mod github_repository_metadata;
mod interact;
mod issue;
mod issue_group;
mod log_file;

use crate::branch_name::BranchName;
use crate::cli::Cli;
use crate::cli::{CommitGrouping, CommitsToConsider, OverlayCommitsIntoOnePullRequest};
use crate::editor::{interactive_get_pr_metadata, PullRequestMetadata};
use crate::github_repository_metadata::GithubRepositoryMetadata;
use crate::interact::{select_issues, IssueGroupWhitelist};
use crate::issue::Issue;
use crate::issue_group::{GitCommitSummary, IssueGroup};

// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.

const PREFIX_PENDING: &str = " ";
const PREFIX_WORKING: &str = ">";
const PREFIX_DONE: &str = "✔";

lazy_static! {
    static ref RE_MULTIPLE_HYPHENS: Regex =
        Regex::new("-{2,}").expect("Expected multiple-hyphens regular expression to compile");
    static ref STYLE_ISSUE_GROUP_STABLE: ProgressStyle =
        ProgressStyle::with_template("{prefix:.green} {msg}").unwrap();
    static ref STYLE_ISSUE_GROUP_WORKING: ProgressStyle =
        ProgressStyle::with_template("{prefix:.yellow} {msg}").unwrap();
    static ref STYLE_COMMIT_STABLE: ProgressStyle =
        ProgressStyle::with_template("  {prefix:.green} {msg}").unwrap();
    static ref STYLE_COMMIT_WORKING: ProgressStyle =
        ProgressStyle::with_template("  {spinner:.yellow} {msg}").unwrap();
}

#[derive(Debug)]
struct CommitPlan<'repo> {
    branch_name: BranchName,
    commits: Vec<Commit<'repo>>,
}

/// Plan out branch names to avoid collisions.
///
/// This function does not take into account existing branch names in the local
/// or remote repository. It only looks at branch names that git-disjoint is
/// going to generate to make sure one invocation of git-disjoint won't try to
/// create a branch with the same name twice.
fn plan_branch_names<'repo>(
    commits_by_issue_group: IndexMap<IssueGroup, Vec<Commit<'repo>>>,
) -> Result<IndexMap<IssueGroup, CommitPlan<'repo>>, anyhow::Error> {
    let mut suffix: u32 = 0;
    let mut seen_branch_names = HashSet::new();
    commits_by_issue_group
        .into_iter()
        .map(|(issue_group, commits)| {
            // Grab the first summary to convert into a branch name.
            // We only choose the first summary because we know each Vec is
            // non-empty and the first element is convenient.
            let summary = {
                let commit = &commits[0];
                commit.summary().ok_or_else(|| {
                    anyhow!(
                        "Summary for commit {:?} is not a valid UTF-8 string",
                        commit.id()
                    )
                })?
            };
            let generated_branch_name = BranchName::from_issue_group(&issue_group, summary);
            let mut proposed_branch_name = generated_branch_name.clone();

            while seen_branch_names.contains(&proposed_branch_name) {
                suffix += 1;
                // OPTIMIZE: no need to call sanitize_git_ref here again
                proposed_branch_name = format!("{generated_branch_name}_{suffix}").into();
            }

            seen_branch_names.insert(proposed_branch_name.clone());

            Ok((
                issue_group,
                CommitPlan {
                    branch_name: proposed_branch_name,
                    commits,
                },
            ))
        })
        .collect()
}

#[derive(Debug)]
struct CommitWork<'repo> {
    commit: Commit<'repo>,
    progress_bar: ProgressBar,
}

impl<'repo> From<Commit<'repo>> for CommitWork<'repo> {
    fn from(commit: Commit<'repo>) -> Self {
        let progress_bar = ProgressBar::new(1)
            .with_style(STYLE_COMMIT_STABLE.clone())
            .with_prefix(PREFIX_PENDING)
            .with_message(commit.summary().unwrap().to_string());
        Self {
            commit,
            progress_bar,
        }
    }
}

#[derive(Debug)]
struct WorkOrder<'repo> {
    branch_name: BranchName,
    commit_work: Vec<CommitWork<'repo>>,
    progress_bar: ProgressBar,
}

impl<'repo> From<(IssueGroup, CommitPlan<'repo>)> for WorkOrder<'repo> {
    fn from((issue_group, commit_plan): (IssueGroup, CommitPlan<'repo>)) -> Self {
        let num_commits: u64 = commit_plan.commits.len().try_into().unwrap();
        let progress_bar = ProgressBar::new(num_commits)
            .with_style(STYLE_ISSUE_GROUP_STABLE.clone())
            .with_prefix(PREFIX_PENDING)
            .with_message(format!("{issue_group}"));
        WorkOrder {
            branch_name: commit_plan.branch_name,
            commit_work: commit_plan
                .commits
                .into_iter()
                .map(CommitWork::from)
                .collect(),
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
    html_url: String,
}

macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(_) => return None,
        }
    };
}

fn execute(command: &[&str], log_file: &Path) -> Result<(), anyhow::Error> {
    let mut runner = Command::new(command[0]);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .unwrap();

    writeln!(file, "$ {:?}", command.join(" "))?;

    // DISCUSS: how to pipe stdout to the same file?
    // Do we need the duct crate?
    // https://stackoverflow.com/a/41025699
    // It's not immediately obvious to me how we pass `command`
    // to a duct `cmd`, but I bet there's a way to separate
    // the head and the tail from our slice.
    runner.stdout(Stdio::null());
    runner.stderr(file);

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
fn assert_repository_state_is_clean(repo: &Repository) -> Result<(), anyhow::Error> {
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
fn assert_tree_matches_workdir_with_index(repo: &Repository) -> Result<(), anyhow::Error> {
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

fn get_base_commit<'repo>(
    repo: &'repo Repository,
    base: &DefaultBranch,
) -> Result<Commit<'repo>, anyhow::Error> {
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
) -> Result<Vec<Commit<'repo>>, anyhow::Error> {
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
) -> Result<IndexMap<IssueGroup, Vec<Commit>>, anyhow::Error> {
    let mut suffix: u32 = 0;
    let mut seen_issue_groups = HashSet::new();
    let commits_by_issue: IndexMap<IssueGroup, Vec<Commit>> = commits
        .into_iter()
        // Parse issue from commit message
        .map(
            |commit| -> Result<Option<(IssueGroup, Commit)>, anyhow::Error> {
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
                // - we're treating every commit separately, or
                // - we're considering all commits (even commits without an issue),
                // add this commit to a unique issue-group.
                if commit_grouping == CommitGrouping::Individual
                    || commits_to_consider == CommitsToConsider::All
                {
                    let summary: GitCommitSummary = (&commit).try_into()?;
                    let mut proposed_issue_group = summary.clone();

                    while seen_issue_groups.contains(&proposed_issue_group) {
                        suffix += 1;
                        proposed_issue_group = GitCommitSummary(format!("{summary}_{suffix}"));
                    }

                    seen_issue_groups.insert(proposed_issue_group.clone());

                    return Ok(Some((IssueGroup::Commit(proposed_issue_group), commit)));
                }

                // Otherwise, skip this commit.
                // FIXME: use writeln! to stderr
                eprintln!(
                    "Warning: ignoring commit without issue trailer: {:?}",
                    commit.id()
                );
                Ok(None)
            },
        )
        // unwrap the Result
        .collect::<Result<Vec<_>, anyhow::Error>>()?
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
            .filter(|(issue_group, _commits)| whitelist.contains(issue_group))
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

async fn cherry_pick<P>(commit: String, log_file: P) -> Result<(), anyhow::Error>
where
    P: AsRef<Path> + 'static,
{
    execute(
        &["git", "cherry-pick", "--allow-empty", &commit],
        log_file.as_ref(),
    )
}

async fn update_spinner(progress_bar: ProgressBar) -> Result<(), anyhow::Error> {
    loop {
        progress_bar.tick();
        tokio::time::sleep(Duration::from_millis(15)).await;
    }
}

async fn sleep(duration: Duration) -> Result<(), anyhow::Error> {
    tokio::time::sleep(duration).await;
    Ok(())
}

async fn create_pull_request(
    http_client: reqwest::Client,
    owner: String,
    name: String,
    forker: String,
    pr_metadata: PullRequestMetadata,
    github_token: String,
    branch_name: BranchName,
    base: DefaultBranch,
) -> Result<(), anyhow::Error> {
    let response: CreatePullRequestResponse = http_client
        .post(format!("https://api.github.com/repos/{owner}/{name}/pulls"))
        .header("User-Agent", "git-disjoint")
        .header("Accept", "application/vnd.github.v3+json")
        .header("Authorization", format!("token {github_token}"))
        .json(&CreatePullRequestRequest {
            title: pr_metadata.title.clone(),
            body: pr_metadata.body.clone(),
            head: format!("{forker}:{branch_name}"),
            base: base.0.clone(),
            draft: true,
        })
        .send()
        .await
        .map_err(|request_error| anyhow!("Error contacting the GitHub API: {request_error}"))?
        .json()
        .await
        .map_err(|response_error| {
            anyhow!("Error parsing the GitHub API response: {response_error}")
        })?;

    Ok(open::that(response.html_url)?)
}

async fn do_git_disjoint(
    exec: TokioTp,
    cli: Cli,
    repository_metadata: GithubRepositoryMetadata,
    base: DefaultBranch,
    log_file: LogFile,
) -> Result<(), anyhow::Error> {
    let (pr_nursery, mut pr_stream) =
        Nursery::<TokioTp, Result<(), anyhow::Error>>::new(exec.clone());

    let Cli {
        all,
        base: _,
        choose,
        dry_run,
        github_token,
        overlay,
        separate,
    } = cli;

    let GithubRepositoryMetadata {
        owner,
        forker,
        remote,
        name,
        root,
        repository,
    } = repository_metadata;

    let base_commit = get_base_commit(&repository, &base)?;
    let commits = get_commits_since_base(&repository, &base_commit)?;
    // REFACTOR: can we use fewer iterators/collects here?
    // We have to make a first pass to determine the issue groups in play
    let commits_by_issue_group = group_commits_by_issue_group(commits, all, separate)?;
    let selected_issue_groups = select_issues(&commits_by_issue_group, choose, overlay)?;
    // Now filter the set of all issue groups to just the whitelisted issue groups
    let commits_by_issue_group =
        filter_issue_groups_by_whitelist(commits_by_issue_group, &selected_issue_groups);
    let commits_by_issue_group = apply_overlay(commits_by_issue_group, overlay);
    let commit_plan_by_issue_group = plan_branch_names(commits_by_issue_group)?;

    let work_orders: Vec<WorkOrder> = commit_plan_by_issue_group
        .into_iter()
        .map(WorkOrder::from)
        .collect();

    // Short-circuit early if there is no work to do.
    if work_orders.is_empty() {
        return Ok(());
    }

    let http_client = reqwest::Client::new();

    let multi_progress_bar = MultiProgress::new();

    for work_order in work_orders.iter() {
        // Insert one progress bar for the issue group.
        multi_progress_bar.insert_from_back(0, work_order.progress_bar.clone());
        work_order.progress_bar.tick();
        // and one progress bar for each ticket.
        // `tick` is necessary to force a repaint
        for commit_work in work_order.commit_work.iter() {
            multi_progress_bar.insert_from_back(0, commit_work.progress_bar.clone());
            commit_work.progress_bar.tick();
        }
    }

    for work_order in work_orders {
        work_order
            .progress_bar
            .set_style(STYLE_ISSUE_GROUP_WORKING.clone());
        work_order.progress_bar.set_prefix(PREFIX_WORKING);
        work_order.progress_bar.tick();

        let branch_ref = format!("refs/heads/{}", work_order.branch_name);
        let branch_obj = repository.revparse_single(&branch_ref);

        // If branch already exists, assume we've already handled this ticket
        // DISCUSS: in the future, we could compare this branch against the desired
        // commits, and add any missing commits to this branch and then update the remote
        if branch_obj.is_ok() {
            eprintln!(
                "Warning: a branch named {:?} already exists",
                work_order.branch_name
            );
            continue;
        }

        if !dry_run {
            // Create a branch
            repository.branch(work_order.branch_name.as_str(), &base_commit, true)?;

            // Check out the new branch
            let branch_obj = repository.revparse_single(&branch_ref)?;
            repository.checkout_tree(&branch_obj, None)?;
            repository.set_head(&branch_ref)?;
        }

        // Cherry-pick commits related to the target issue
        for commit_work in work_order.commit_work.iter() {
            commit_work
                .progress_bar
                .set_style(STYLE_COMMIT_WORKING.clone());

            // If we need to update the UI and perform a blocking action, spawn
            // a worker thread. If there's no work to do, we can keep the UI
            // activity on the main thread. But we don't, so the dry_run flag
            // exercises more of the same code paths as a live run does.
            let (nursery, mut output) =
                Nursery::<TokioTp, Result<(), anyhow::Error>>::new(exec.clone());
            nursery.nurse(update_spinner(commit_work.progress_bar.clone()))?;

            if dry_run {
                nursery.nurse(sleep(Duration::from_millis(750)))?;
            } else {
                let commit_hash = commit_work.commit.id().to_string();
                nursery.nurse(cherry_pick(commit_hash, log_file.clone()))?;
            }

            // Prevent new tasks from spawning
            drop(nursery);
            // Wait for the cherry-pick to terminate
            output.try_next().await?;
            // Cancel the infinite spinner
            drop(output);

            commit_work
                .progress_bar
                .set_style(STYLE_COMMIT_STABLE.clone());
            commit_work.progress_bar.set_prefix(PREFIX_DONE);
            commit_work.progress_bar.finish()
        }

        if !dry_run {
            // Push the branch
            execute(
                &["git", "push", &remote, (work_order.branch_name.as_str())],
                log_file.as_ref(),
            )?;

            // Open a pull request
            // Only ask the user to edit the PR metadata when multiple commits
            // create ambiguity about the contents of the PR title and body.
            let needs_edit = work_order.commit_work.len() > 1;

            let pr_metadata = match needs_edit {
                true => interactive_get_pr_metadata(
                    &root,
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
                        body: commit.body().unwrap_or_default().to_owned(),
                    }
                }
            };

            pr_nursery.nurse(create_pull_request(
                http_client.clone(),
                owner.clone(),
                name.clone(),
                forker.clone(),
                pr_metadata,
                github_token.clone(),
                work_order.branch_name.clone(),
                base.clone(),
            ))?;

            // Finally, check out the original ref
            execute(&["git", "checkout", "-"], log_file.as_ref())?;
        }

        work_order
            .progress_bar
            .set_style(STYLE_ISSUE_GROUP_STABLE.clone());
        work_order.progress_bar.set_prefix(PREFIX_DONE);
        work_order.progress_bar.finish();
    }

    drop(pr_nursery);
    while pr_stream.try_next().await?.is_some() {}

    Ok(())
}

fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let exec: TokioTp = TokioTpBuilder::new().build()?;

    let program = async {
        let log_file = LogFile::default();
        let repository_metadata = GithubRepositoryMetadata::try_default()?;
        let base_branch = cli.base.clone();
        let base_branch = base_branch
            .map(DefaultBranch)
            .ok_or_else(|| anyhow!("user has not provided a default branch"));
        let base_branch = match base_branch {
            Ok(base) => Ok(base),
            Err(_) => DefaultBranch::try_get_default(&repository_metadata, &cli.github_token).await,
        }?;

        assert_repository_state_is_clean(&repository_metadata.repository)?;
        assert_tree_matches_workdir_with_index(&repository_metadata.repository)?;

        // TODO: rename for clarity
        let result = do_git_disjoint(
            exec.clone(),
            cli.clone(),
            repository_metadata,
            base_branch,
            log_file.clone(),
        )
        .await;
        match result {
            // Execution succeeded, so clean up the log file
            Ok(()) => Ok(log_file.delete()?),
            Err(err) => {
                // Execution failed, so display the logs and the error to the user
                let log_contents = fs::read_to_string(&log_file)?;
                indoc::eprintdoc!(
                    "
                    Failed with error {:?}

                    Full log output:
                    {}

                    The log file is {:?}
                    ",
                    err,
                    log_contents,
                    log_file,
                );
                Ok(())
            }
        }
    };

    exec.block_on(program)
}
