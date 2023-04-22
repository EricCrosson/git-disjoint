use clap::{ArgAction, Parser};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommitsToConsider {
    All,
    WithTrailer,
}

impl From<&str> for CommitsToConsider {
    fn from(value: &str) -> Self {
        match value {
            "true" => Self::All,
            "false" => Self::WithTrailer,
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum PromptUserToChooseCommits {
    Yes,
    No,
}

impl From<&str> for PromptUserToChooseCommits {
    fn from(value: &str) -> Self {
        match value {
            "true" => Self::Yes,
            "false" => Self::No,
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum OverlayCommitsIntoOnePullRequest {
    Yes,
    No,
}

impl From<&str> for OverlayCommitsIntoOnePullRequest {
    fn from(value: &str) -> Self {
        match value {
            "true" => Self::Yes,
            "false" => Self::No,
            _ => unreachable!(),
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommitGrouping {
    Individual,
    ByIssue,
}

impl From<&str> for CommitGrouping {
    fn from(value: &str) -> Self {
        match value {
            "true" => Self::Individual,
            "false" => Self::ByIssue,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug, Parser)]
#[command(author, version, about)]
pub(crate) struct Cli {
    /// Do not ignore commits without an issue trailer.
    ///
    /// Commits without an issue trailer are considered to be their own
    /// group, so will be the only commit in their PR.
    ///
    /// There is no change to commits with an issue trailer.
    ///
    /// This flag can be combined with the --choose flag.
    #[clap(
        short,
        long,
        help = "Consider every commit, even commits without an issue trailer",
        action = ArgAction::SetTrue,
    )]
    pub all: CommitsToConsider,

    /// The starting point (exclusive) of commits to act on.
    ///
    /// Defaults to the repository's default branch.
    #[clap(
        short,
        long,
        help = "The starting point (exclusive) of commits to act on",
        value_name = "REF"
    )]
    pub base: Option<String>,

    /// Prompt the user to select which issues to create PRs for.
    ///
    /// Select a whitelist of issues (or commits, if the --all flag is active)
    /// in a terminal UI.
    #[clap(
        short,
        long,
        help = "Prompt the user to select which issues to create PRs for",
        action = ArgAction::SetTrue,
    )]
    pub choose: PromptUserToChooseCommits,

    /// Show the work that would be performed without taking any action.
    #[clap(
        short,
        long,
        env = "GIT_DISJOINT_DRY_RUN",
        help = "Show the work that would be performed without taking any action"
    )]
    pub dry_run: bool,

    /// GitHub API token with repo permissions.
    #[clap(
        long,
        env = "GITHUB_TOKEN",
        help = "GitHub API token",
        value_name = "TOKEN"
    )]
    pub github_token: String,

    /// Combine multiple issue groups into one PR.
    ///
    /// When this flag is active, git-disjoint will create only one PR.
    ///
    /// This can be useful when you have multiple commits with no trailer that
    /// would be better reviewed or merged together.
    #[clap(
        short,
        long,
        help = "Combine multiple issue groups into one PR",
        action = ArgAction::SetTrue,
    )]
    pub overlay: OverlayCommitsIntoOnePullRequest,

    /// Do not group commits by issue.
    ///
    /// Treat each commit independently, regardless of issue trailer. Each
    /// PR created will have one and only one commit associated with it.
    ///
    /// This is the same behavior as when no commit has an issue trailer and
    /// the --all flag is active.
    ///
    /// This can be useful when you have numerous changes that belong under
    /// one issue, but would be better reviewed independently.
    #[clap(
        short,
        long,
        help = "Treat every commit separately; do not group by issue",
        action = ArgAction::SetTrue,
    )]
    pub separate: CommitGrouping,
}
