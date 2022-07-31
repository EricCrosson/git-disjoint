#![forbid(unsafe_code)]

use std::collections::HashMap;

use anyhow::Result;
use clap::Parser;
use git2::{Commit, Repository};

mod args;
mod issue;

use crate::args::Args;
use crate::issue::Issue;

// DISCUSS: enforcing prerequisite: working tree is clean
// DISCUSS: how to handle cherry-pick merge conflicts, and resuming gracefully
// What if we stored a log of what we were going to do before we took any action?
// Or kept it as a list of things to do, removing successful items.
// TODO: add documentation
// TODO: semantic-release to cargo

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
            HashMap::<Issue, Vec<Commit>>::new(),
            |mut map, (issue, commit)| {
                let commits = map.entry(issue).or_default();
                commits.push(commit);
                map
            },
        );

    // DEBUG:
    println!("{:#?}", commits_by_issue);

    // RESUME: for each issue: create a branch
    // NEXT: cherry-pick the related commits
    // NEXT: push the branch
    // NEXT: open a pull request

    Ok(())
}
