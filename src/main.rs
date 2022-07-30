#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use git2::{Commit, Repository};

mod args;
mod issue;

use crate::args::Args;
use crate::issue::Issue;

fn main() -> Result<()> {
    let Args { start_point } = Args::parse();
    let repo = Repository::open(".")?;
    let mut revwalk = repo.revwalk()?;

    // Assume `revspec` indicates a single commit
    let start_point = repo.revparse(&start_point)?;

    // Traverse commits starting from HEAD
    revwalk.push_head()?;

    revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)?;

    // Filter our revwalk based on the CLI parameters
    macro_rules! filter_try {
        ($e:expr) => {
            match $e {
                Ok(t) => t,
                Err(e) => return None,
            }
        };
    }

    let revwalk = revwalk
        .filter_map(|id| {
            let id = filter_try!(id);
            let commit = filter_try!(repo.find_commit(id));

            Some(commit)
        })
        // Skip all commits before the `start_point`
        .skip_while(|commit| match start_point.from() {
            Some(start_point_oid) => !start_point_oid.id().eq(&commit.id()),
            None => true,
        })
        // Parse into a tuple of (Ticket, Commit)
        .map(|commit| -> (Option<Issue>, Commit) {
            let issue = commit.message().and_then(Issue::parse_from_commit_message);
            (issue, commit)
        })
        .for_each(|(issue, commit)| {
            println!("{:#?}", commit);
        });

    Ok(())
}
