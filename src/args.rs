#![deny(missing_docs)]

use clap::{crate_version, Parser};

use crate::default_branch::DefaultBranch;

#[derive(Parser)]
#[clap(
    name = "git-disjoint",
    author,
    version = crate_version!(),
    about,
)]
pub(crate) struct Args {
    #[clap(short, long)]
    pub since: Option<DefaultBranch>,
}
