#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::env;
use std::path::Path;

use anyhow::{anyhow, Result};
use clap::Parser;
use git2::{Commit, Cred, Direction, PushOptions, RemoteCallbacks, Repository};
use lazy_static::lazy_static;
use regex::Regex;

mod args;
mod issue;
mod sanitize_git_branch_name;

use crate::args::Args;
use crate::issue::Issue;
use crate::sanitize_git_branch_name::sanitize_text_for_git_branch_name;

// DISCUSS: enforcing prerequisite: working tree is clean
// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.
// TODO: add documentation
// TODO: semantic-release to cargo

#[derive(Debug)]
struct PullRequestContent<'a> {
    issue: Issue,
    commits: Vec<Commit<'a>>,
}

macro_rules! filter_try {
    ($e:expr) => {
        match $e {
            Ok(t) => t,
            Err(_) => return None,
        }
    };
}

fn main() -> Result<()> {
    let Args { start_point } = Args::parse();
    let repo = Repository::open(".")?;
    let mut revwalk = repo.revwalk()?;

    // Assume `revspec` indicates a single commit
    let start_point = repo.revparse_single(&start_point)?;
    let start_point_commit = start_point
        .as_commit()
        .ok_or(anyhow!("Expected start_point to identify a commit"))?;

    // Traverse commits starting from HEAD
    revwalk.push_head()?;

    revwalk.set_sorting(git2::Sort::TOPOLOGICAL)?;

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

    // DEBUG:
    // println!("Commits: {:#?}", commits);

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
            // REFACTOR: avoid a tuple here, use a struct for readability
            HashMap::<Issue, Vec<Commit>>::new(),
            |mut map, (issue, commit)| {
                let commits = map.entry(issue).or_default();
                commits.push(commit);
                map
            },
        );

    // DEBUG:
    // println!("{:#?}", commits_by_issue);

    // DEBUG:
    commits_by_issue
        .into_iter()
        .map(|(issue, commits)| PullRequestContent { issue, commits })
        // DEBUG:
        .take(1)
        .try_for_each(|pair| -> Result<()> {
            // DEBUG:
            println!("{:#?}", pair);

            // RESUME: for each issue: create a branch
            let PullRequestContent { issue, commits } = pair;
            let summary = commits[0]
                .summary()
                .ok_or(anyhow!("Commit summary is not valid UTF-8"))?;

            // Replace parentheses, because they interfere with terminal tab-completion
            // (they require double quotes).
            let branch_name = {
                let branch_name =
                    sanitize_text_for_git_branch_name(&format!("{}-{}", issue, summary))
                        .replace("(", "-")
                        .replace(")", "-");
                lazy_static! {
                    static ref RE_MULTIPLE_HYPHENS: Regex = Regex::new("-{2,}")
                        .expect("Expected multiple-hyphens regular expression to compile");
                }
                RE_MULTIPLE_HYPHENS
                    .replace_all(&branch_name, "-")
                    .to_string()
            };

            repo.branch(&branch_name, start_point_commit, true)?;

            // Check out the new branch
            let branch_ref = format!("refs/heads/{}", &branch_name);
            let branch_obj = repo.revparse_single(&branch_ref)?;
            repo.checkout_tree(&branch_obj, None)?;
            repo.set_head(&branch_ref)?;

            // Cherry-pick commits related to the target issue
            // Helpful resource: https://github.com/rust-lang/git2-rs/pull/432/files
            for commit in commits {
                // DEBUG:
                println!("Cherry-picking commit {}", &commit.id());

                let parent = repo
                    .find_branch(&branch_name, git2::BranchType::Local)?
                    .get()
                    .peel_to_commit()?;
                // DEBUG:
                println!("Branch peels to commit {:?}", parent,);

                repo.cherrypick(&commit, None)?;

                let id = repo.index()?.write_tree()?;
                let tree_d = repo.find_tree(id)?;
                repo.commit(
                    Some("HEAD"),
                    &commit.author(),
                    &commit.committer(),
                    &commit.message().unwrap_or_default(),
                    &tree_d,
                    &[&parent],
                )?;
            }

            // Without knowing how to properly commit a cherry-pick,
            // we have to manually clear the repo state.
            repo.cleanup_state()?;

            // NEXT: push the branch
            let mut remote_callbacks = RemoteCallbacks::new();
            remote_callbacks.credentials(|_url, username_from_url, _allowed_types| {
                Cred::ssh_key(
                    username_from_url.unwrap(),
                    None,
                    Path::new(&format!("{}/.ssh/id_rsa", env::var("HOME").unwrap())),
                    None,
                )
            });

            // let mut push_options = PushOptions::new();
            // push_options.remote_callbacks(remote_callbacks);

            let mut remote = repo.find_remote("origin")?;
            remote.connect_auth(Direction::Push, Some(remote_callbacks), None)?;
            remote.push(
                &[format!("{}:{}", &branch_ref, &branch_ref)],
                // Some(&mut push_options),
                None,
            )?;

            // NEXT: open a pull request
            // NEXT: check out the original branch

            Ok(())
        })?;

    Ok(())
}
