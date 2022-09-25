# Git Disjoint

[![Build Status]](https://github.com/EricCrosson/git-disjoint/actions/workflows/ci.yml)
[![Crates.io]](https://crates.io/crates/git-disjoint)

[build status]: https://github.com/EricCrosson/git-disjoint/actions/workflows/ci.yml/badge.svg
[crates.io]: https://img.shields.io/crates/v/git-disjoint.svg

<p align="center">
  <img src="https://github.com/EricCrosson/git-disjoint/blob/master/assets/logo.png?raw=true" alt="alt-text"/>
</p>

**git-disjoint** automates an optimal git workflow for PR authors and reviewers
by grouping commits by issue onto unique branches, referencing issues in
branch names, and creating PRs.

This encourages the submission of small, independent PRs, minimizing cognitive
load on reviewers and keeping cycle time low.

## How does it work?

**git-disjoint** uses commit messages to determine which issue a commit relates
to. Following formalized conventions for commit messages, **git-disjoint**
automatically creates a PR for each issue and associates the PR to an existing
issue in your work tracker.

When the PR merges, the existing issue closes and your git history updates to
reflect the upstream changes.

## Supported Integrations

`git-disjoint` may add value to your workflow if you

- use a work tracker (currently supports Jira and GitHub Issues)
- use GitHub and Pull Requests

## Installing

### Cargo

If `cargo` is installed on your system, run:

```
cargo +nightly install git-disjoint
```

### Manual

Otherwise, download a release compatible with your OS and architecture from the
[Releases] page, extract the binary, and put it somewhere in your `$PATH`.

[releases]: https://github.com/EricCrosson/git-disjoint/releases/latest

## Making commits

1. [Add all of your commits to the repository's default branch][workflow].

1. In each commit message, include a reference to the relevant ticket.

   For example, use the Jira automation [format][jira]:

   ```
   Ticket: COOL-123
   ```

   or

   ```
   Closes Ticket: COOL-123
   ```

   Or use the GitHub [format][gh]: 

    ```
    Closes #123
    ```

   [jira]: https://support.atlassian.com/jira-software-cloud/docs/reference-issues-in-your-development-work/
   [gh]: https://github.blog/2013-01-22-closing-issues-via-commit-messages/

## Opening PRs

 When you're ready to:

1. turn the set of commits addressing each ticket into its own feature branch,
1. push that branch, and 
1. create a draft PR,

run `git disjoint`.

[workflow]: https://drewdevault.com/2020/04/06/My-weird-branchless-git-workflow.html

## Ignoring commits

To ignore commits associated with an issue, use the `--choose` flag. This will
open a menu where you can select the issues to create PRs for.
