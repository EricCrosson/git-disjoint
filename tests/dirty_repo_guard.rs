use std::path::Path;

use git2::{Repository as Git2Repository, Signature, Time};
use tempfile::TempDir;

use git_disjoint::git2_repository::Repository;

fn fixed_signature() -> Signature<'static> {
    Signature::new("Test User", "test@test.com", &Time::new(1_000_000_000, 0)).unwrap()
}

fn init_repo_with_commit(dir: &Path) -> Git2Repository {
    let repo = Git2Repository::init(dir).unwrap();
    let sig = fixed_signature();

    std::fs::write(dir.join("file.txt"), "initial content").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("file.txt")).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    {
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap();
    }
    repo
}

#[test]
fn clean_repo_succeeds() {
    let tempdir = TempDir::new().unwrap();
    let _git2_repo = init_repo_with_commit(tempdir.path());

    let result = Repository::try_from(tempdir.path());
    assert!(result.is_ok(), "clean repo should succeed");
}

#[test]
fn staged_changes_rejected() {
    let tempdir = TempDir::new().unwrap();
    let git2_repo = init_repo_with_commit(tempdir.path());

    // Stage a new file without committing
    std::fs::write(tempdir.path().join("staged.txt"), "staged").unwrap();
    let mut index = git2_repo.index().unwrap();
    index.add_path(Path::new("staged.txt")).unwrap();
    index.write().unwrap();

    let result = Repository::try_from(tempdir.path());
    assert!(result.is_err(), "staged changes should be rejected");
}

#[test]
fn unstaged_changes_rejected() {
    let tempdir = TempDir::new().unwrap();
    let _git2_repo = init_repo_with_commit(tempdir.path());

    // Modify a tracked file without staging
    std::fs::write(tempdir.path().join("file.txt"), "modified content").unwrap();

    let result = Repository::try_from(tempdir.path());
    assert!(result.is_err(), "unstaged changes should be rejected");
}

#[test]
fn nonexistent_path_rejected() {
    let result = Repository::try_from(Path::new("/tmp/nonexistent-git-disjoint-test-path"));
    assert!(result.is_err(), "nonexistent path should be rejected");
}
