# Git Disjoint

[![Build Status]](https://github.com/EricCrosson/git-disjoint/actions/workflows/ci.yml)
[![Crates.io]](https://crates.io/crates/git-disjoint)

[build status]: https://github.com/EricCrosson/git-disjoint/actions/workflows/ci.yml/badge.svg
[crates.io]: https://img.shields.io/crates/v/git-disjoint.svg

<p align="center">
  <img src="https://github.com/EricCrosson/git-disjoint/blob/master/assets/logo.png?raw=true" alt="alt-text"/>
</p>

`git disjoint` groups commits by issue onto unique branches.

## Installing

#### Cargo

```
cargo +nightly install git-disjoint
```

#### Manual

Download a release compatible with your OS and architecture from the [Releases] page, extract the binary, and put it somewhere in your `$PATH`.

[releases]: https://github.com/EricCrosson/git-disjoint/releases/latest

## Assumptions

`git disjoint` may add value to your workflow if you

- use a work tracker (currently supports Jira)
- use GitHub and Pull Requests

## Goals

`git disjoint` automates referencing issues in your development work ([Jira]) so you can focus on development.

[Jira]: https://support.atlassian.com/jira-software-cloud/docs/reference-issues-in-your-development-work/

## Workflow

1. [Add all your commits to one branch].

1. In each commit message, include a reference to the relevant ticket.

   For example, use the Jira automation format:

   ```
   Ticket: COOL-123
   ```

   or

   ```
   Closes Ticket: COOL-123
   ```

   Or use the GitHub [format]: 

    ```
    Closes 123
    ```

   [format]: https://github.blog/2013-01-22-closing-issues-via-commit-messages/

1. When you're ready to:

   1. turn the set of commits addressing each ticket into its own feature branch,
   1. push that branch, and 
   1. create a draft PR,

   run `git disjoint`.

[add all your commits to one branch]: https://drewdevault.com/2020/04/06/My-weird-branchless-git-workflow.html
[git-branchless]: https://github.com/arxanas/git-branchless

## Caveats

- There's currently no code to handle the case where `git disjoint` tries to operate on a branch that already exists. This can happen if you invoke `git disjoint` twice on the same branch. See [#32].

[#32]: https://github.com/EricCrosson/git-disjoint/issues/32
