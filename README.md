# Git Disjoint

<p align="center">
  <img src="https://github.com/EricCrosson/git-disjoint/blob/master/assets/logo.png?raw=true" alt="alt-text"/>
</p>

`git disjoint` groups commits by issue onto unique branches.

## Installing

#### Cargo

```
cargo install git-disjoint
```

## Assumptions

`git disjoint` may add value to your workflow if you

- use a work tracker (currently supports Jira)
- use GitHub and Pull Requests

## Goals

`git disjoint` automates referencing issues in your development work ([Jira]) so you can focus on development.

[Jira]: https://support.atlassian.com/jira-software-cloud/docs/reference-issues-in-your-development-work/

## Workflow

1. [Add all your commits to one branch].

   Starting from your repository's default branch, this could look like:

   ```
   git checkout -b now
   ```

   or, if you are using [git-branchless]:

   ```
   git checkout --detach
   ```

1. In each commit message, include a reference to the relevant ticket.

   For example:

   ```
   Ticket: COOL-123
   ```

   or

   ```
   Closes Ticket: COOL-123
   ```

2. When you're ready to turn the set of commits addressing each ticket into its own feature branch, push that branch, and create a draft PR, run `git disjoint`.

[add all your commits to one branch]: https://drewdevault.com/2020/04/06/My-weird-branchless-git-workflow.html
[git-branchless]: https://github.com/arxanas/git-branchless

## Caveats

- There's currently no code to handle the case where `git disjoint` tries to operate on a branch that already exists. This can happen if you invoke `git disjoint` twice on the same branch. See [#32].

[#32]: https://github.com/EricCrosson/git-disjoint/issues/32
