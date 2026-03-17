use std::fmt::Write;

use git2::Commit;

use crate::branch_name::BranchName;
use crate::disjoint_branch::DisjointBranchMap;

#[derive(Debug)]
pub struct BranchConflict {
    pub branch_name: BranchName,
    pub commit_summary: String,
    pub conflicting_paths: Vec<String>,
}

#[derive(Debug)]
pub struct PreValidationReport {
    pub conflicts: Vec<BranchConflict>,
}

impl PreValidationReport {
    pub fn render(&self, _use_color: bool) -> String {
        let mut output = String::new();
        for (i, conflict) in self.conflicts.iter().enumerate() {
            if i > 0 {
                writeln!(output).unwrap();
            }
            writeln!(
                output,
                "error: cherry-pick would fail for branch `{}`",
                conflict.branch_name
            )
            .unwrap();
            writeln!(output, "  --> commit \"{}\"", conflict.commit_summary).unwrap();
            writeln!(output, "   |").unwrap();
            for path in &conflict.conflicting_paths {
                writeln!(output, "   = conflict in {}", path).unwrap();
            }
            writeln!(output, "   |").unwrap();
            writeln!(
                output,
                "   = help: these commits have overlapping changes and cannot be split"
            )
            .unwrap();
            writeln!(
                output,
                "           into separate branches from the same base"
            )
            .unwrap();
            writeln!(
                output,
                "   = help: consider assigning them to the same issue, or use `--overlay`"
            )
            .unwrap();
            writeln!(output, "           to combine them into a single PR").unwrap();
        }
        output
    }
}

pub fn validate<'repo>(
    branch_map: &DisjointBranchMap<'repo>,
    base_commit: &Commit<'repo>,
    repo: &git2::Repository,
) -> Result<(), PreValidationReport> {
    let mut conflicts = Vec::new();

    for (_issue_group, branch) in branch_map.iter() {
        let mut simulated_head = base_commit.clone();

        for commit in &branch.commits {
            let mut index = repo
                .cherrypick_commit(commit, &simulated_head, 0, None)
                .map_err(|_| PreValidationReport {
                    conflicts: vec![BranchConflict {
                        branch_name: branch.branch_name.clone(),
                        commit_summary: commit.summary().unwrap_or("").to_string(),
                        conflicting_paths: vec!["(git2 error)".to_string()],
                    }],
                })?;

            if index.has_conflicts() {
                let conflicting_paths: Vec<String> = index
                    .conflicts()
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|conflict| {
                        let conflict = conflict.ok()?;
                        conflict
                            .our
                            .or(conflict.their)
                            .or(conflict.ancestor)
                            .map(|entry| String::from_utf8_lossy(&entry.path).to_string())
                    })
                    .collect();

                conflicts.push(BranchConflict {
                    branch_name: branch.branch_name.clone(),
                    commit_summary: commit.summary().unwrap_or("").to_string(),
                    conflicting_paths,
                });
                // Stop simulating this branch after first conflict
                break;
            } else {
                // Advance simulated head
                let tree_oid = index.write_tree_to(repo).map_err(|_| PreValidationReport {
                    conflicts: vec![BranchConflict {
                        branch_name: branch.branch_name.clone(),
                        commit_summary: commit.summary().unwrap_or("").to_string(),
                        conflicting_paths: vec!["(write_tree error)".to_string()],
                    }],
                })?;
                let tree = repo.find_tree(tree_oid).map_err(|_| PreValidationReport {
                    conflicts: vec![BranchConflict {
                        branch_name: branch.branch_name.clone(),
                        commit_summary: commit.summary().unwrap_or("").to_string(),
                        conflicting_paths: vec!["(find_tree error)".to_string()],
                    }],
                })?;
                let sig = commit.author();
                simulated_head = repo
                    .find_commit(
                        repo.commit(None, &sig, &sig, "simulated", &tree, &[&simulated_head])
                            .map_err(|_| PreValidationReport {
                                conflicts: vec![BranchConflict {
                                    branch_name: branch.branch_name.clone(),
                                    commit_summary: commit.summary().unwrap_or("").to_string(),
                                    conflicting_paths: vec!["(commit error)".to_string()],
                                }],
                            })?,
                    )
                    .map_err(|_| PreValidationReport {
                        conflicts: vec![BranchConflict {
                            branch_name: branch.branch_name.clone(),
                            commit_summary: commit.summary().unwrap_or("").to_string(),
                            conflicting_paths: vec!["(find_commit error)".to_string()],
                        }],
                    })?;
            }
        }
    }

    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(PreValidationReport { conflicts })
    }
}
