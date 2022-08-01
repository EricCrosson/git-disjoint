#![forbid(unsafe_code)]

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use clap::Parser;
use git2::{Commit, Repository};
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
    let start_point = repo.revparse(&start_point)?;
    let start_point_commit = start_point
        .from()
        .ok_or(anyhow!("Expected start_point to identify a single commit"))?
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
        .take_while(|commit| match start_point.from() {
            Some(start_point_oid) => !start_point_oid.id().eq(&commit.id()),
            None => true,
        })
        .collect();

    // Order commits parent-first, children-last
    commits.reverse();

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
    let pair = &commits_by_issue
        .into_iter()
        .fold(Vec::new(), |mut vec, (issue, commits)| {
            vec.push(PullRequestContent { issue, commits });
            vec
        })[0];
    // DEBUG:
    println!("{:#?}", pair);

    // RESUME: for each issue: create a branch
    let PullRequestContent { issue, commits } = pair;
    let summary = commits[0]
        .summary()
        .ok_or(anyhow!("Commit summary is not valid UTF-8"))?;

    // Replace parentheses, because they interfere with terminal tab-completion
    // (they require double quotes).
    let branch_name = sanitize_text_for_git_branch_name(&format!("{}-{}", issue, summary));
    let branch_name = branch_name.replace("(", "-");
    let branch_name = branch_name.replace(")", "-");
    lazy_static! {
        static ref RE_MULTIPLE_HYPHENS: Regex =
            Regex::new("-{2,}").expect("Expected multiple-hyphens regular expression to compile");
    }
    let branch_name = RE_MULTIPLE_HYPHENS.replace_all(&branch_name, "-");
    repo.branch(&branch_name, start_point_commit, true)?;

    // Check out the new branch
    let branch_obj = repo.revparse_single(&("refs/heads/".to_owned() + &branch_name))?;
    repo.checkout_tree(&branch_obj, None)?;
    repo.set_head(&("refs/heads/".to_owned() + &branch_name))?;

    // NEXT: cherry-pick the related commits
    // NEXT: push the branch
    // NEXT: open a pull request

    Ok(())
}
