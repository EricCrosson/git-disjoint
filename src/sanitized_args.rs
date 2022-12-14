use anyhow::{anyhow, Result};
use clap::Parser;

use crate::args::Args;
use crate::default_branch::DefaultBranch;

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
                .map(|s| DefaultBranch(s))
                .ok_or_else(|| anyhow!("User has not provided a default branch"))
                .or_else(|_| DefaultBranch::try_get_default())?,
            choose,
            all,
            separate,
        })
    }
}
