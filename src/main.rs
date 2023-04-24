#![forbid(unsafe_code)]
#![feature(exit_status_error)]

use std::fs::{self, OpenOptions};
use std::io::prelude::*;
use std::process::{Command, Stdio};
use std::time::Duration;

use async_executors::{TokioTp, TokioTpBuilder};
use async_nursery::{NurseExt, Nursery};
use clap::Parser;
use default_branch::DefaultBranch;
use disjoint_branch::{DisjointBranch, DisjointBranchMap};
use futures::TryStreamExt;
use git2::Commit;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use issue_group_map::IssueGroupMap;
use lazy_static::lazy_static;
use log_file::LogFile;
use pull_request::PullRequest;

mod branch_name;
mod cli;
mod default_branch;
mod disjoint_branch;
mod editor;
mod git2_repository;
mod github_repository_metadata;
mod increment;
mod interact;
mod issue;
mod issue_group;
mod issue_group_map;
mod log_file;
mod pull_request;
mod pull_request_message;
mod pull_request_metadata;

use crate::branch_name::BranchName;
use crate::cli::Cli;
use crate::editor::interactive_get_pr_metadata;
use crate::github_repository_metadata::GithubRepositoryMetadata;
use crate::issue_group::IssueGroup;

// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.

const PREFIX_PENDING: &str = " ";
const PREFIX_WORKING: &str = ">";
const PREFIX_DONE: &str = "✔";

lazy_static! {
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
struct CommitWork<'repo> {
    commit: Commit<'repo>,
    progress_bar: ProgressBar,
}

impl<'repo> Into<&Commit<'repo>> for &'repo CommitWork<'repo> {
    fn into(self) -> &'repo Commit<'repo> {
        &self.commit
    }
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

impl<'repo> From<(IssueGroup, DisjointBranch<'repo>)> for WorkOrder<'repo> {
    fn from((issue_group, commit_plan): (IssueGroup, DisjointBranch<'repo>)) -> Self {
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

fn execute(command: &[&str], log_file: &LogFile) -> Result<(), anyhow::Error> {
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

async fn cherry_pick(commit: String, log_file: LogFile) -> Result<(), anyhow::Error> {
    execute(&["git", "cherry-pick", "--allow-empty", &commit], &log_file)
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

async fn do_git_disjoint(exec: TokioTp, cli: Cli, log_file: LogFile) -> Result<(), anyhow::Error> {
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

    let repository_metadata = GithubRepositoryMetadata::try_default()?;
    let base_branch = cli.base.clone();
    let base_branch = match base_branch {
        Some(base) => DefaultBranch(base),
        None => DefaultBranch::try_get_default(&repository_metadata, &github_token).await?,
    };

    let GithubRepositoryMetadata {
        owner,
        forker,
        remote,
        name,
        root,
        repository,
    } = repository_metadata;

    let base_commit = repository.base_commit(&base_branch)?;
    let commits = repository.commits_since_base(&base_commit)?;
    // We have to make a first pass to determine the issue groups in play
    let commits_by_issue_group = IssueGroupMap::try_from_commits(commits, all, separate)?
        // Now filter the set of all issue groups to just the whitelisted issue groups
        .select_issues(choose, overlay)?
        .apply_overlay(overlay);

    let commit_plan_by_issue_group: DisjointBranchMap = commits_by_issue_group.try_into()?;

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
                &log_file,
            )?;

            // Open a pull request
            // Only ask the user to edit the PR metadata when multiple commits
            // create ambiguity about the contents of the PR title and body.
            let needs_edit = work_order.commit_work.len() > 1;

            let pr_metadata = match needs_edit {
                true => interactive_get_pr_metadata(&root, &work_order.commit_work)?,
                false => {
                    // REFACTOR: clean this up
                    let commit = &work_order.commit_work.get(0).unwrap().commit;
                    commit.message().unwrap().parse()?
                }
            };

            let pull_request = PullRequest {
                owner: owner.clone(),
                name: name.clone(),
                forker: forker.clone(),
                title: pr_metadata.title,
                body: pr_metadata.body,
                github_token: github_token.clone(),
                branch_name: work_order.branch_name.clone(),
                base: base_branch.clone(),
            };

            pr_nursery.nurse(pull_request.create(http_client.clone()))?;

            // Finally, check out the original ref
            execute(&["git", "checkout", "-"], &log_file)?;
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

        // TODO: rename for clarity
        let result = do_git_disjoint(exec.clone(), cli.clone(), log_file.clone()).await;
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
