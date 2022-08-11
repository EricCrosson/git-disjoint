use clap::{crate_version, Parser};

use crate::default_branch::DefaultBranch;

#[derive(Parser)]
#[clap(name = "git-disjoint", version = crate_version!(), author = "Eric Crosson <eric.s.crosson@utexas.edu>")]
pub(crate) struct Args {
    #[clap(short, long)]
    pub(crate) since: Option<DefaultBranch>,
}
