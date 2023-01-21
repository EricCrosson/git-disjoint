#[derive(clap::Parser)]
#[command(author, version, about)]
pub(crate) struct Args {
    /// Do not ignore commits without an issue footer.
    ///
    /// Commits without an issue footer are considered to be their own
    /// group, so will be the only commit in their PR.
    ///
    /// There is no change to commits with an issue footer.
    ///
    /// This flag can be combined with the --choose flag.
    #[clap(
        short,
        long,
        help = "Consider every commit, even commits without an issue footer"
    )]
    pub all: bool,

    /// The starting point (exclusive) of commits to act on.
    ///
    /// Defaults to the repository's default branch.
    #[clap(
        short,
        long,
        help = "The starting point (exclusive) of commits to act on"
    )]
    pub base: Option<String>,

    /// Prompt the user to select which issues to create PRs for.
    ///
    /// Select a whitelist of issues (or commits, if the --all flag is active)
    /// in a terminal UI.
    #[clap(
        short,
        long,
        help = "Prompt the user to select which issues to create PRs for"
    )]
    pub choose: bool,

    /// Show the work that would be performed without taking any action.
    #[clap(
        short,
        long,
        help = "Show the work that would be performed without taking any action"
    )]
    pub dry_run: bool,

    /// Combine multiple issue groups into one PR.
    ///
    /// When this flag is active, git-disjoint will create only one PR.
    ///
    /// This can be useful when you have multiple commits with no footer that
    /// would be better reviewed or merged together.
    #[clap(short, long, help = "Combine multiple issue groups into one PR")]
    pub overlay: bool,

    /// Do not group commits by issue.
    ///
    /// Treat each commit independently, regardless of issue footer. Each
    /// PR created will have one and only one commit associated with it.
    ///
    /// This is the same behavior as when no commit has an issue footer and
    /// the --all flag is active.
    ///
    /// This can be useful when you have numerous changes that belong under
    /// one issue, but would be better reviewed independently.
    #[clap(
        short,
        long,
        help = "Treat every commit separately; do not group by issue"
    )]
    pub separate: bool,
}
