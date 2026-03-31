// Phase 3: Shell detection via parent process walk

use std::path::Path;

/// Extract basename from a path string (e.g., "/usr/bin/bash" -> "bash").
/// Strips leading '-' from login shells (e.g., "-bash" -> "bash").
pub fn basename_from_path(path: &str) -> String {
    let name = Path::new(path.trim())
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.trim().to_string());
    name.trim_start_matches('-').to_string()
}

/// Detect the shell by examining the parent process.
pub fn detect() -> String {
    // Try to detect from parent process
    if let Some(shell) = detect_from_parent() {
        return shell;
    }

    // Fallback cascade: environment variables
    if std::env::var("BASH_VERSION").is_ok() {
        return "bash".to_string();
    }
    if std::env::var("ZSH_VERSION").is_ok() {
        return "zsh".to_string();
    }
    if std::env::var("FISH_VERSION").is_ok() {
        return "fish".to_string();
    }

    // Final fallback: $SHELL
    if let Ok(shell) = std::env::var("SHELL") {
        let name = basename_from_path(&shell);
        if !name.is_empty() {
            return name;
        }
    }

    "unknown".to_string()
}

fn detect_from_parent() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let ppid = std::os::unix::process::parent_id();
        let exe_path = format!("/proc/{}/exe", ppid);
        if let Ok(target) = std::fs::read_link(&exe_path) {
            let name = basename_from_path(&target.to_string_lossy());
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        let ppid = std::os::unix::process::parent_id();
        if let Ok(output) = std::process::Command::new("ps")
            .args(["-p", &ppid.to_string(), "-o", "comm="])
            .output()
        {
            if output.status.success() {
                let name = basename_from_path(
                    &String::from_utf8_lossy(&output.stdout),
                );
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_absolute_path() {
        assert_eq!(basename_from_path("/usr/bin/bash"), "bash");
    }

    #[test]
    fn basename_just_name() {
        assert_eq!(basename_from_path("zsh"), "zsh");
    }

    #[test]
    fn basename_login_shell() {
        assert_eq!(basename_from_path("-bash"), "bash");
    }

    #[test]
    fn basename_login_shell_absolute() {
        assert_eq!(basename_from_path("/bin/-zsh"), "zsh");
    }

    #[test]
    fn basename_with_whitespace() {
        assert_eq!(basename_from_path("  /usr/bin/fish  "), "fish");
    }

    #[test]
    fn basename_empty() {
        assert_eq!(basename_from_path(""), "");
    }

    #[test]
    fn detect_returns_nonempty() {
        let shell = detect();
        assert!(!shell.is_empty());
        assert_ne!(shell, "unknown"); // In test harness, we should at least get $SHELL
    }
}
