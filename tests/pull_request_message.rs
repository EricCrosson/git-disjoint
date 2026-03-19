mod common;

use std::path::Path;

use git2::{Oid, Repository as Git2Repository, Signature, Time};
use tempfile::TempDir;

use git_disjoint::default_branch::DefaultBranch;
use git_disjoint::git2_repository::Repository;
use git_disjoint::pull_request_message::PullRequestMessageTemplate;

fn fixed_signature() -> Signature<'static> {
    Signature::new("Test User", "test@test.com", &Time::new(1_000_000_000, 0)).unwrap()
}

fn make_commit(repo: &Git2Repository, parent: Oid, filename: &str, message: &str) -> Oid {
    let sig = fixed_signature();
    std::fs::write(repo.workdir().unwrap().join(filename), filename).unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(filename)).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let parent_commit = repo.find_commit(parent).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])
        .unwrap()
}

#[test]
fn editor_template_orders_commits_most_recent_first() {
    let tempdir = TempDir::new().unwrap();
    let git2_repo = Git2Repository::init(tempdir.path()).unwrap();
    let sig = fixed_signature();

    // Create base commit
    std::fs::write(tempdir.path().join(".gitkeep"), "").unwrap();
    let mut index = git2_repo.index().unwrap();
    index.add_path(Path::new(".gitkeep")).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let base_oid = {
        let tree = git2_repo.find_tree(tree_oid).unwrap();
        git2_repo
            .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap()
    };

    // Register the remote tracking ref that git-disjoint uses as the base
    git2_repo
        .reference("refs/remotes/origin/main", base_oid, true, "test setup")
        .unwrap();

    // Create 3 commits: oldest A, then B, then C (most recent)
    let a = make_commit(
        &git2_repo,
        base_oid,
        "a.txt",
        "feat: commit A\n\nTicket: AB-1",
    );
    let b = make_commit(&git2_repo, a, "b.txt", "feat: commit B\n\nTicket: AB-1");
    let _c = make_commit(&git2_repo, b, "c.txt", "feat: commit C\n\nTicket: AB-1");

    let repo: Repository = git2_repo.into();
    let base_commit = repo
        .base_commit(&DefaultBranch("main".to_string()))
        .unwrap();

    let commits: Vec<git2::Commit> = repo.commits_since_base(&base_commit).unwrap().collect();
    let template: PullRequestMessageTemplate = commits.iter().collect();

    insta::assert_snapshot!(format!("{template}"));
}
