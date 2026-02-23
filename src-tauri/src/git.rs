use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: String,
    pub default_branch: String,
    pub latest_commit_hash: String,
    pub latest_commit_time: String,
    pub has_unstaged_changes: bool,
    pub has_staged_changes: bool,
    pub is_git_repo: bool,
}

/// Validate that a repo path is absolute and is a directory (no traversal).
fn validate_repo_path(repo_path: &str) -> bool {
    let path = Path::new(repo_path);
    path.is_absolute() && path.is_dir()
}

/// Get git information for a repository
pub fn get_git_info(repo_path: &str) -> GitInfo {
    if !validate_repo_path(repo_path) {
        return GitInfo::default();
    }

    // Check if it's a git repo
    let is_git_repo = run_git_command(repo_path, &["rev-parse", "--git-dir"]).is_some();
    if !is_git_repo {
        return GitInfo::default();
    }

    let branch = get_current_branch(repo_path).unwrap_or_default();
    let default_branch = get_default_branch(repo_path);
    let (latest_commit_hash, latest_commit_time) = get_latest_commit(repo_path);
    let has_unstaged_changes = check_unstaged_changes(repo_path);
    let has_staged_changes = check_staged_changes(repo_path);

    GitInfo {
        branch,
        default_branch,
        latest_commit_hash,
        latest_commit_time,
        has_unstaged_changes,
        has_staged_changes,
        is_git_repo: true,
    }
}

fn run_git_command(repo_path: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(["-C", repo_path])
        .args(args)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

fn get_current_branch(repo_path: &str) -> Option<String> {
    run_git_command(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
}

fn get_latest_commit(repo_path: &str) -> (String, String) {
    let hash = run_git_command(repo_path, &["rev-parse", "--short", "HEAD"]).unwrap_or_default();

    let time = run_git_command(repo_path, &["log", "-1", "--format=%cr"]).unwrap_or_default();

    (hash, time)
}

fn check_unstaged_changes(repo_path: &str) -> bool {
    // Check for unstaged changes in tracked files using git diff
    let has_tracked_changes = Command::new("git")
        .args(["-C", repo_path, "diff", "--quiet"])
        .status()
        .map(|s| !s.success()) // exit code 1 means there are changes
        .unwrap_or(false);

    // Check for untracked files
    let has_untracked = run_git_command(repo_path, &["ls-files", "--others", "--exclude-standard"])
        .map(|output| !output.is_empty())
        .unwrap_or(false);

    has_tracked_changes || has_untracked
}

fn check_staged_changes(repo_path: &str) -> bool {
    // Check for staged changes using git diff --cached
    Command::new("git")
        .args(["-C", repo_path, "diff", "--cached", "--quiet"])
        .status()
        .map(|s| !s.success()) // exit code 1 means there are staged changes
        .unwrap_or(false)
}

/// Get list of local branches for a repository
pub fn get_branches(repo_path: &str) -> Vec<String> {
    if !validate_repo_path(repo_path) {
        return Vec::new();
    }

    run_git_command(repo_path, &["branch", "--format=%(refname:short)"])
        .map(|output| {
            output
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn get_default_branch(repo_path: &str) -> String {
    // 1. Try to get default branch from remote HEAD (if remote exists)
    if let Some(remote_head) =
        run_git_command(repo_path, &["symbolic-ref", "refs/remotes/origin/HEAD"])
    {
        // Output is like "refs/remotes/origin/main"
        if let Some(branch) = remote_head.strip_prefix("refs/remotes/origin/") {
            if !branch.is_empty() {
                return branch.to_string();
            }
        }
    }

    // 2. Check git config for init.defaultBranch setting
    if let Some(config_default) = run_git_command(repo_path, &["config", "init.defaultBranch"]) {
        if !config_default.is_empty() {
            return config_default;
        }
    }

    // 3. Check if common default branches exist locally
    for branch in ["main", "master", "develop"] {
        if run_git_command(repo_path, &["rev-parse", "--verify", branch]).is_some() {
            return branch.to_string();
        }
    }

    // 4. Get the first local branch as last resort
    if let Some(branches) = run_git_command(repo_path, &["branch", "--format=%(refname:short)"]) {
        if let Some(first_branch) = branches.lines().next() {
            let trimmed = first_branch.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    // 5. Fallback to main if nothing works
    "main".to_string()
}
