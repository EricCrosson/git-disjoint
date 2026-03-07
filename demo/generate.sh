#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Ensure VHS is available
if ! command -v vhs &>/dev/null; then
  echo "error: vhs is not installed. Run from: nix develop ./nix" >&2
  exit 1
fi

# Build git-disjoint in release mode
echo "Building git-disjoint..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml"

# Create isolated demo environment
DEMO_DIR=$(mktemp -d)
trap 'rm -rf "$DEMO_DIR"' EXIT

# Initialize a git repo to serve as the demo environment
git -c init.defaultBranch=master init "$DEMO_DIR/repo"
cd "$DEMO_DIR/repo"
git commit --allow-empty -m "initial commit"
git remote add origin https://github.com/example/demo.git

# Copy prepare-commits into the demo repo
cp "$SCRIPT_DIR/prepare-commits" "$DEMO_DIR/repo/prepare-commits"
chmod +x "$DEMO_DIR/repo/prepare-commits"

# Create wrapper directory
WRAPPER_DIR="$DEMO_DIR/bin"
mkdir -p "$WRAPPER_DIR"

# Create a git-disjoint wrapper that enables dry-run mode and injects --base
# to avoid a GitHub API call for determining the default branch.
cat > "$WRAPPER_DIR/git-disjoint" << WRAPPER
#!/usr/bin/env bash
export GIT_DISJOINT_DRY_RUN=true
export GITHUB_TOKEN=demo
exec "$REPO_ROOT/target/release/git-disjoint" --base master "\$@"
WRAPPER
chmod +x "$WRAPPER_DIR/git-disjoint"

# Shadow bash with a wrapper that sets a clean PS1 prompt.
# VHS validates shell names against a hardcoded list and rejects custom names,
# so we shadow "bash" on PATH to inject our prompt configuration.
REAL_BASH="$(command -v bash)"
cat > "$WRAPPER_DIR/bash" << SHELL
#!/bin/sh
export PS1='\$ '
exec "$REAL_BASH" --norc --noprofile "\$@"
SHELL
chmod +x "$WRAPPER_DIR/bash"

# Record the demo with the wrappers on PATH
echo "Recording demo..."
cd "$DEMO_DIR/repo"
PATH="$WRAPPER_DIR:$PATH" \
  vhs "$SCRIPT_DIR/demo.tape" -o "$SCRIPT_DIR/demo.gif"

echo "Done! GIF written to demo/demo.gif"
