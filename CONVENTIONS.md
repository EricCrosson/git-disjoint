# Git-Disjoint Conventions

## Project Overview

Git-disjoint is a tool that helps developers manage multiple related commits across different issues without manual branch management.
It identifies commits by their issue references in commit messages, groups them appropriately, and creates pull requests automatically.

## Core Functionality

1. **Issue Identification**: Git-disjoint parses commit messages for issue references (e.g., "Fixes #123" or "JIRA-456") to group related commits.

2. **Automatic Branch Creation**: Creates branches for each issue group without requiring manual branch management.

3. **Pull Request Generation**: Automatically creates pull requests for each issue group, linking them to the appropriate issue tracker items.

## Workflow Example

1. Developer makes several commits directly to their local master branch
2. Each commit includes proper issue references in the commit message
3. When ready to submit changes, developer runs `git-disjoint`
4. The tool identifies related commits, creates branches, and submits PRs
5. After PRs are merged, developer pulls changes, advancing the default branch pointer beyond the local commits, concluding the workflow.

## Development Guidelines

1. **Commit Messages**: Include issue references as trailers in commit messages (e.g., "Fixes #123")
2. **Error Handling**: Use modular errors, as described in [Modular Errors in Rust](https://sabrinajewson.org/blog/errors). Never use the `anyhow` crate - the project should use custom, well-defined error types that provide clear context and follow the modular error pattern.
3. **Testing**: Write tests for all functionality. Avoid unnecessary coupling of tests to internal implementation details, so refactoring the implementation does not break tests. Tests should be written from the user's perspective and exhaustively cover happy paths and edge cases. Practice test-driven development.
4. **Code Style**: Follow Rust idioms. Use ports and adapters, implemented as traits and impls. Use a cargo workspace, organized as described in https://matklad.github.io/2021/08/22/large-rust-workspaces.html
5. **Usability**: The primary goal of this package is to offer a supreme developer experience. This means beautiful output, clear error messages, and simulating an operation before attempting it, to prevent leaving the user's git repository in an unfamiliar, broken state.
6. **Dependencies**: It is a goal to reduce the number of production dependencies to zero, or as near to it as is practical. Stability of any existing dependencies is paramount, minimizing the need to update dependencies and risk breaking changes or manual intervention to adapt to a new crate API.
