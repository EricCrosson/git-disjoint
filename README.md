# git disjoint

[![Build Status]](https://github.com/EricCrosson/git-disjoint/actions/workflows/release.yml)

[build status]: https://github.com/EricCrosson/git-disjoint/actions/workflows/release.yml/badge.svg?event=push

<p align="center">
  <img src="https://github.com/EricCrosson/git-disjoint/blob/master/assets/logo.png?raw=true" alt="conceptual diagram of git-disjoint operation"/>
</p>

**git-disjoint** automates an optimal git workflow for PR authors and reviewers
by grouping commits by issue into GitHub PRs.

This encourages the submission of small, independent PRs, minimizing cognitive
load on reviewers, maximizing the utility of your git history, and keeping
cycle time low.

## Elevator Pitch

<p align="center">
  <img src="https://i.imgur.com/bUuxI8c.gif" alt="git-disjoint demo">
</p>

You're working on a feature. As you work, you create some commits that don't directly
implement your feature. Maybe you improve some documentation, fix a minor bug, or
first refactor to make the change easy, before making the easy change[^1]. In any case,
you commit directly to master[^2] as you go, because you want each change to persist
in your development environment, even before it's gone through code review and landed
upstream.

When you come to a natural stopping point, you are ready to ship several
commits. Each commit is atomic, relating to just one topic. It comes with a
detailed commit message referencing an issue in your work tracker, passing
unit tests, and documentation. You don't want to shove all these changes into
a single PR, because they deal with orthogonal concerns. You trust your team to
contribute quality code reviews, and iterating on one changeset shouldn't delay
unrelated changes from merging.

Instead of creating a PR directly from your master, or manually moving commits into separate
branches, do this:

```shell
git disjoint
```

**git-disjoint** will identify which commits relate to the same issue, batch these commits
into a new branch, and create a PR.

[^1]: https://www.adamtal.me/2019/05/first-make-the-change-easy-then-make-the-easy-change
[^2]: https://drewdevault.com/2020/04/06/My-weird-branchless-git-workflow.html

## How does it work?

**git-disjoint** looks for trailers[^3] in each commit message to determine
which issue a commit relates to. By default, it creates one PR for each issue
and associates the PR to an existing issue in your work tracker.

When a PR merges, your next `git pull` effectively moves upstream's master from
behind your local commits to ahead of them.

[^3]: https://git-scm.com/docs/git-interpret-trailers

## Supported Integrations

**git-disjoint** adds value to your workflow if you:

- use a work tracker (supports Jira and GitHub Issues)
- use GitHub and Pull Requests

## Requirements

You must have the [gh] command installed and configured.

[gh]: https://github.com/cli/cli

## Install

### From GitHub releases

The easiest way to install **git-disjoint** is to download a release compatible
with your OS and architecture from the [Releases] page.

Alternatively, install **git-disjoint** with one of the following package managers:

| Repository     | Command                                               |
| -------------- | ----------------------------------------------------- |
| Cargo          | `cargo +nightly install git-disjoint`                 |
| Cargo binstall | `cargo binstall git-disjoint`                         |
| Nix            | `nix profile install github:EricCrosson/git-disjoint` |

[releases]: https://github.com/EricCrosson/git-disjoint/releases/latest

## Use

### Make commits

1. Add all of your commits to a single branch. I recommend using the repository's default branch.

1. In each commit message, include a reference to the relevant issue.

   For example, use the Jira automation [format][jira]:

   ```
   Ticket: COOL-123
   ```

   or

   ```
   Closes Ticket: COOL-123
   ```

   Or use the GitHub [format][github]:

   ```
   Closes #123
   ```

[jira]: https://support.atlassian.com/jira-software-cloud/docs/reference-issues-in-your-development-work/
[github]: https://github.blog/2013-01-22-closing-issues-via-commit-messages/

### Open PRs

When you're ready to:

1. turn the set of commits addressing each issue into its own branch,
1. push that branch, and
1. create a draft PR,

run `git disjoint`.

## How-to Guide

### How do I ignore certain commits?

To ignore commits associated with an issue, use the `--choose` flag. This will
open a menu where you can select the issues to create PRs for.

### How do I use git-disjoint on commits without an associated issue?

Use the `--all` flag to include commits without a recognized trailer.
