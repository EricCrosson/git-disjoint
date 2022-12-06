use anyhow::{anyhow, Result};
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
    /// The lower-bound (exclusive) of commits to act on
    #[clap(short, long)]
    pub base: Option<DefaultBranch>,

    /// Prompt the user to select which issues to create PRs for
    #[clap(short, long, action)]
    pub choose: bool,

    /// Create a branch for all commits, even without an associated issue
    #[clap(short, long, action)]
    pub all: bool,

    /// Treat every commit separately; do not group by ticket
    #[clap(short, long, action)]
    pub separate: bool,
}

pub(crate) struct SanitizedArgs {
    pub base: DefaultBranch,
    pub choose: bool,
    pub all: bool,
    pub separate: bool,
}

impl SanitizedArgs {
    pub(crate) fn parse() -> Result<SanitizedArgs> {
        Args::parse().try_into()
    }
}

impl TryFrom<Args> for SanitizedArgs {
    type Error = anyhow::Error;

    fn try_from(value: Args) -> Result<Self, Self::Error> {
        let Args {
            base,
            choose,
            all,
            separate,
        } = value;
        Ok(Self {
            // Clap doesn't provide a way to supply a default value coming from
            // a function when the user has not supplied a required value.
            // This TryFrom bridges the gap.
            base: base
                .ok_or_else(|| anyhow!("User has not provided a default branch"))
                .or_else(|_| DefaultBranch::try_get_default())?,
            choose,
            all,
            separate,
        })
    }
}
