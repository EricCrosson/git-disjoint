use std::collections::BTreeMap;
use std::path::Path;

use git2::{Commit, Oid, Repository, Signature, Time};
use tempfile::TempDir;

use git_disjoint::cli::{CommitGrouping, CommitsToConsider, OverlayCommitsIntoOnePullRequest};
use git_disjoint::disjoint_branch::DisjointBranchMap;
use git_disjoint::issue_group_map::IssueGroupMap;
use git_disjoint::pre_validation;

const FIXED_TIME: i64 = 1_000_000_000;
const FIXED_OFFSET: i32 = 0;

#[derive(Debug)]
pub struct TestFixture {
    pub title: String,
    pub base_files: BTreeMap<String, String>,
    pub commits: Vec<TestCommit>,
    pub run_args: Vec<String>,
}

#[derive(Debug)]
pub struct TestCommit {
    pub message: String,
    pub files: BTreeMap<String, String>,
    pub delete: Vec<String>,
}

pub fn parse_fixture(kdl: &str) -> TestFixture {
    let doc: kdl::KdlDocument = kdl.parse().expect("invalid KDL");

    let title = doc
        .get("title")
        .expect("fixture must have a title")
        .entries()
        .first()
        .expect("title must have a value")
        .value()
        .as_string()
        .expect("title must be a string")
        .to_string();

    let mut base_files = BTreeMap::new();
    if let Some(base_node) = doc.get("base") {
        if let Some(children) = base_node.children() {
            for node in children.nodes() {
                if node.name().value() == "file" {
                    let entries: Vec<_> = node.entries().iter().collect();
                    let path = entries[0]
                        .value()
                        .as_string()
                        .expect("file path must be a string")
                        .to_string();
                    let content = entries[1]
                        .value()
                        .as_string()
                        .expect("file content must be a string")
                        .to_string();
                    base_files.insert(path, content);
                }
            }
        }
    }

    let mut commits = Vec::new();
    for node in doc.nodes() {
        if node.name().value() == "commit" {
            let message = node
                .entries()
                .first()
                .expect("commit must have a message")
                .value()
                .as_string()
                .expect("commit message must be a string")
                .to_string();

            let mut files = BTreeMap::new();
            let mut delete = Vec::new();

            if let Some(children) = node.children() {
                for child in children.nodes() {
                    match child.name().value() {
                        "file" => {
                            let entries: Vec<_> = child.entries().iter().collect();
                            let path = entries[0]
                                .value()
                                .as_string()
                                .expect("file path must be a string")
                                .to_string();
                            let content = entries[1]
                                .value()
                                .as_string()
                                .expect("file content must be a string")
                                .to_string();
                            files.insert(path, content);
                        }
                        "delete" => {
                            let path = child
                                .entries()
                                .first()
                                .expect("delete must have a path")
                                .value()
                                .as_string()
                                .expect("delete path must be a string")
                                .to_string();
                            delete.push(path);
                        }
                        _ => {}
                    }
                }
            }

            commits.push(TestCommit {
                message,
                files,
                delete,
            });
        }
    }

    let run_node = doc.get("run").expect("fixture must have a run node");
    let run_str = run_node
        .entries()
        .first()
        .expect("run must have a value")
        .value()
        .as_string()
        .expect("run value must be a string");

    let run_args = parse_cli_args(run_str);

    TestFixture {
        title,
        base_files,
        commits,
        run_args,
    }
}

fn parse_cli_args(run_str: &str) -> Vec<String> {
    let mut tokens = run_str.split_whitespace();
    let cmd = tokens.next().expect("run string must not be empty");
    assert_eq!(cmd, "git-disjoint", "run command must be 'git-disjoint'");
    tokens.map(|s| s.to_string()).collect()
}

fn resolve_cli_args(
    args: &[String],
) -> (
    CommitsToConsider,
    CommitGrouping,
    OverlayCommitsIntoOnePullRequest,
) {
    let mut all = CommitsToConsider::WithTrailer;
    let mut separate = CommitGrouping::ByIssue;
    let mut overlay = OverlayCommitsIntoOnePullRequest::No;

    for arg in args {
        match arg.as_str() {
            "--all" | "-a" => all = CommitsToConsider::All,
            "--separate" | "-s" => separate = CommitGrouping::Individual,
            "--overlay" | "-o" => overlay = OverlayCommitsIntoOnePullRequest::Yes,
            other => panic!("unknown fixture arg: {other}"),
        }
    }

    (all, separate, overlay)
}

fn fixed_signature() -> Signature<'static> {
    Signature::new(
        "Test User",
        "test@test.com",
        &Time::new(FIXED_TIME, FIXED_OFFSET),
    )
    .unwrap()
}

fn write_file(repo_path: &Path, relative_path: &str, content: &str) {
    let full_path = repo_path.join(relative_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&full_path, content).unwrap();
}

fn delete_file(repo_path: &Path, relative_path: &str) {
    let full_path = repo_path.join(relative_path);
    std::fs::remove_file(full_path).unwrap();
}

fn create_commit(
    repo: &Repository,
    parent: &Commit,
    files: &BTreeMap<String, String>,
    deletes: &[String],
    message: &str,
) -> Oid {
    let repo_path = repo.workdir().unwrap();
    let sig = fixed_signature();
    let mut index = repo.index().unwrap();

    for (path, content) in files {
        write_file(repo_path, path, content);
        index.add_path(Path::new(path)).unwrap();
    }

    for path in deletes {
        delete_file(repo_path, path);
        index.remove_path(Path::new(path)).unwrap();
    }

    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[parent])
        .unwrap()
}

struct TestRepo {
    _tempdir: TempDir,
    repo: Repository,
    base_commit_oid: Oid,
}

fn build_test_repo(fixture: &TestFixture) -> TestRepo {
    let tempdir = TempDir::new().unwrap();
    let repo = Repository::init(tempdir.path()).unwrap();

    let sig = fixed_signature();

    // Create initial commit with base files
    let mut index = repo.index().unwrap();

    // Always create at least one file so the base commit has a tree
    if fixture.base_files.is_empty() {
        write_file(tempdir.path(), ".gitkeep", "");
        index.add_path(Path::new(".gitkeep")).unwrap();
    } else {
        for (path, content) in &fixture.base_files {
            write_file(tempdir.path(), path, content);
            index.add_path(Path::new(path)).unwrap();
        }
    }

    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();

    let base_oid = {
        let tree = repo.find_tree(tree_oid).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
            .unwrap()
    };

    // Create the remote ref that git-disjoint looks for
    repo.reference("refs/remotes/origin/main", base_oid, true, "test setup")
        .unwrap();

    // Now apply each test commit
    let mut parent_oid = base_oid;
    for test_commit in &fixture.commits {
        let parent = repo.find_commit(parent_oid).unwrap();
        parent_oid = create_commit(
            &repo,
            &parent,
            &test_commit.files,
            &test_commit.delete,
            &test_commit.message,
        );
    }

    TestRepo {
        _tempdir: tempdir,
        repo,
        base_commit_oid: base_oid,
    }
}

pub fn run_fixture(fixture: &TestFixture) -> String {
    let test_repo = build_test_repo(fixture);
    let (all, separate, overlay) = resolve_cli_args(&fixture.run_args);

    let base_commit = test_repo
        .repo
        .find_commit(test_repo.base_commit_oid)
        .unwrap();

    // Walk commits since base
    let mut revwalk = test_repo.repo.revwalk().unwrap();
    revwalk.push_head().unwrap();
    revwalk.hide(base_commit.id()).unwrap();
    revwalk
        .set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::REVERSE)
        .unwrap();

    let commits: Vec<Commit> = revwalk
        .filter_map(|id| {
            let id = id.ok()?;
            test_repo.repo.find_commit(id).ok()
        })
        .collect();

    // Build the issue group map
    let commits_by_issue_group = match IssueGroupMap::try_from_commits(commits, all, separate) {
        Ok(map) => map.apply_overlay(overlay),
        Err(e) => return format!("exit: 1\n\nerror: {e}"),
    };

    // Build the branch map
    let branch_map: DisjointBranchMap = match commits_by_issue_group.try_into() {
        Ok(map) => map,
        Err(e) => return format!("exit: 1\n\nerror: {e}"),
    };

    // Validate
    if let Err(report) = pre_validation::validate(&branch_map, &base_commit, &test_repo.repo) {
        return format!("exit: 1\n\n{}", report.render(false).trim_end());
    }

    if branch_map.is_empty() {
        return "exit: 0\n\n(no branches planned)".to_string();
    }

    // Execute: create branches via in-memory cherry-pick and render per-branch
    use std::fmt::Write;
    let mut output = "exit: 0".to_string();

    for (_issue_group, branch) in branch_map.iter() {
        let mut simulated_head = base_commit.clone();

        for commit in &branch.commits {
            let mut index = test_repo
                .repo
                .cherrypick_commit(commit, &simulated_head, 0, None)
                .unwrap();

            let tree_oid = index.write_tree_to(&test_repo.repo).unwrap();
            let tree = test_repo.repo.find_tree(tree_oid).unwrap();
            let sig = fixed_signature();
            let new_oid = test_repo
                .repo
                .commit(
                    None,
                    &sig,
                    &sig,
                    commit.summary().unwrap_or(""),
                    &tree,
                    &[&simulated_head],
                )
                .unwrap();
            simulated_head = test_repo.repo.find_commit(new_oid).unwrap();
        }

        // Create a branch ref pointing at the final commit
        test_repo
            .repo
            .branch(branch.branch_name.as_str(), &simulated_head, true)
            .unwrap();

        // Render branch header
        write!(output, "\n\nbranch {}:", branch.branch_name).unwrap();
        for commit in &branch.commits {
            write!(output, "\n  * {}", commit.summary().unwrap_or("")).unwrap();
        }

        // List files in the branch's tree
        let tree = simulated_head.tree().unwrap();
        let mut files: Vec<String> = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |dir, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                let path = if dir.is_empty() {
                    entry.name().unwrap().to_string()
                } else {
                    format!("{}{}", dir, entry.name().unwrap())
                };
                if path != ".gitkeep" {
                    files.push(path);
                }
            }
            git2::TreeWalkResult::Ok
        })
        .unwrap();
        files.sort();

        write!(output, "\n  files: {}", files.join(", ")).unwrap();
    }

    output
}
