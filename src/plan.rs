use std::fmt::Write;

use crate::branch_name::BranchName;
use crate::disjoint_branch::DisjointBranchMap;
use crate::issue_group::IssueGroup;

pub struct Plan {
    pub branches: Vec<PlannedBranch>,
}

pub struct PlannedBranch {
    pub branch_name: BranchName,
    pub issue_group: IssueGroup,
    pub commits: Vec<PlannedCommit>,
}

pub struct PlannedCommit {
    pub oid: git2::Oid,
    pub summary: String,
}

impl Plan {
    pub fn from_disjoint_branch_map(map: &DisjointBranchMap) -> Self {
        let branches = map
            .iter()
            .map(|(issue_group, branch)| PlannedBranch {
                branch_name: branch.branch_name.clone(),
                issue_group: issue_group.clone(),
                commits: branch
                    .commits
                    .iter()
                    .map(|commit| PlannedCommit {
                        oid: commit.id(),
                        summary: commit.summary().unwrap_or("").to_string(),
                    })
                    .collect(),
            })
            .collect();
        Plan { branches }
    }

    pub fn render(&self) -> String {
        let mut output = String::new();
        for (i, branch) in self.branches.iter().enumerate() {
            if i > 0 {
                writeln!(output).unwrap();
            }
            writeln!(output, "branch {}:", branch.branch_name).unwrap();
            for commit in &branch.commits {
                writeln!(output, "  * {}", commit.summary).unwrap();
            }
        }
        output
    }
}
