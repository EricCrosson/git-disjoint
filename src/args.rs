use clap::{crate_version, Parser};

#[derive(Parser)]
#[clap(
    name = "git-disjoint",
    author,
    version = crate_version!(),
    about,
)]
pub(crate) struct Args {
    /// The lower-bound (exclusive) of commits to act on
    #[clap(short, long)]
    pub base: Option<String>,

    /// Prompt the user to select which issues to create PRs for
    #[clap(short, long, action)]
    pub choose: bool,

    /// Create a branch for all commits, even without an associated issue
    #[clap(short, long, action)]
    pub all: bool,

    /// Treat every commit separately; do not group by ticket
    #[clap(
        short,
        long,
        action,
        help = "Treat every commit separately; do not group by ticket",
        long_help
    )]
    pub separate: bool,
}
