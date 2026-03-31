// Phase 3: Git repo/branch/dirty detection

use std::process::Command;

pub struct GitInfo {
    pub git_repo: bool,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

/// Parse `git rev-parse --is-inside-work-tree` output.
pub fn parse_is_repo(output: &str) -> bool {
    output.trim() == "true"
}

/// Parse `git rev-parse --abbrev-ref HEAD` output.
/// Returns None for empty, converts "HEAD" (detached) to None so caller
/// can try the short hash fallback.
pub fn parse_branch(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if trimmed.is_empty() || trimmed == "HEAD" {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Parse `git status --porcelain` output to determine if repo is dirty.
pub fn parse_dirty(output: &str) -> bool {
    !output.trim().is_empty()
}

pub fn detect() -> GitInfo {
    // Check if we're inside a git repo
    let is_repo = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .ok()
        .map(|o| o.status.success() && parse_is_repo(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or(false);

    if !is_repo {
        return GitInfo {
            git_repo: false,
            git_branch: None,
            git_dirty: None,
        };
    }

    // Get branch name
    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                parse_branch(&String::from_utf8_lossy(&o.stdout))
            } else {
                None
            }
        })
        .or_else(|| {
            // Fallback for detached HEAD: get short hash
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        let hash = String::from_utf8_lossy(&o.stdout).trim().to_string();
                        if hash.is_empty() { None } else { Some(hash) }
                    } else {
                        None
                    }
                })
        });

    // Check dirty status
    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| {
            if o.status.success() {
                parse_dirty(&String::from_utf8_lossy(&o.stdout))
            } else {
                false
            }
        });

    GitInfo {
        git_repo: true,
        git_branch: branch,
        git_dirty: dirty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_is_repo_true() {
        assert!(parse_is_repo("true\n"));
    }

    #[test]
    fn parse_is_repo_false() {
        assert!(!parse_is_repo("false\n"));
    }

    #[test]
    fn parse_is_repo_empty() {
        assert!(!parse_is_repo(""));
    }

    #[test]
    fn parse_branch_main() {
        assert_eq!(parse_branch("main\n"), Some("main".to_string()));
    }

    #[test]
    fn parse_branch_feature() {
        assert_eq!(
            parse_branch("feature/my-branch\n"),
            Some("feature/my-branch".to_string())
        );
    }

    #[test]
    fn parse_branch_detached() {
        assert_eq!(parse_branch("HEAD\n"), None);
    }

    #[test]
    fn parse_branch_empty() {
        assert_eq!(parse_branch(""), None);
    }

    #[test]
    fn parse_dirty_clean() {
        assert!(!parse_dirty(""));
        assert!(!parse_dirty("\n"));
    }

    #[test]
    fn parse_dirty_modified() {
        assert!(parse_dirty(" M src/main.rs\n"));
    }

    #[test]
    fn parse_dirty_untracked() {
        assert!(parse_dirty("?? new_file.rs\n"));
    }

    #[test]
    fn parse_dirty_multiple() {
        assert!(parse_dirty(" M src/main.rs\n?? new_file.rs\n"));
    }

    #[test]
    fn detect_runs_in_git_repo() {
        // This test runs inside the tai repo, so git_repo should be true
        let info = detect();
        assert!(info.git_repo);
        assert!(info.git_branch.is_some());
        assert!(info.git_dirty.is_some());
    }
}
