use clap::{crate_version, Parser};

#[derive(Parser)]
#[clap(name = "git-disjoint", version = crate_version!(), author = "Eric Crosson <eric.s.crosson@utexas.edu>")]
pub(crate) struct Args {
    // TODO: discuss defaulting to the last merge commit?
    #[clap(value_parser)]
    pub(crate) start_point: String,
}
