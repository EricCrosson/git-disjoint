use anyhow::{anyhow, Result};
use clap::Parser;

use crate::args::Args;
use crate::default_branch::DefaultBranch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitsToConsider {
    All,
    WithTrailer,
}

impl From<bool> for CommitsToConsider {
    fn from(value: bool) -> Self {
        match value {
            true => Self::All,
            false => Self::WithTrailer,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PromptUserToChooseCommits {
    Yes,
    No,
}

impl From<bool> for PromptUserToChooseCommits {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Yes,
            false => Self::No,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OverlayCommitsIntoOnePullRequest {
    Yes,
    No,
}

impl From<bool> for OverlayCommitsIntoOnePullRequest {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Yes,
            false => Self::No,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommitGrouping {
    Individual,
    ByIssue,
}

impl From<bool> for CommitGrouping {
    fn from(value: bool) -> Self {
        match value {
            true => Self::Individual,
            false => Self::ByIssue,
        }
    }
}

pub(crate) struct SanitizedArgs {
    pub all: CommitsToConsider,
    pub base: DefaultBranch,
    pub choose: PromptUserToChooseCommits,
    pub dry_run: bool,
    pub overlay: OverlayCommitsIntoOnePullRequest,
    pub separate: CommitGrouping,
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
            all,
            base,
            choose,
            dry_run,
            overlay,
            separate,
        } = value;
        Ok(Self {
            all: all.into(),
            // Clap doesn't provide a way to supply a default value coming from
            // a function when the user has not supplied a required value.
            // This TryFrom bridges the gap.
            base: base
                .map(|s| DefaultBranch(s))
                .ok_or_else(|| anyhow!("User has not provided a default branch"))
                .or_else(|_| DefaultBranch::try_get_default())?,
            choose: choose.into(),
            dry_run,
            overlay: overlay.into(),
            separate: separate.into(),
        })
    }
}
