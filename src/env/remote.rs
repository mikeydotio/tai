// Phase 3: SSH/mosh detection

/// Detect remote session from env var values and optional process tree check.
/// `ssh_connection`: value of $SSH_CONNECTION
/// `is_mosh_in_tree`: whether mosh-server was found in process ancestry (Linux only)
pub fn detect_from_env(ssh_connection: Option<&str>, is_mosh_in_tree: bool) -> Option<String> {
    if ssh_connection.is_some_and(|v| !v.is_empty()) {
        return Some("ssh".to_string());
    }
    if is_mosh_in_tree {
        return Some("mosh".to_string());
    }
    None
}

/// Walk the process tree on Linux looking for mosh-server.
#[cfg(target_os = "linux")]
fn check_mosh_in_tree() -> bool {
    let mut pid = std::process::id();
    // Walk up to 32 levels to avoid infinite loops
    for _ in 0..32 {
        if pid <= 1 {
            break;
        }
        // Check the comm (process name)
        let comm_path = format!("/proc/{}/comm", pid);
        if let Ok(comm) = std::fs::read_to_string(&comm_path) {
            if comm.trim() == "mosh-server" {
                return true;
            }
        } else {
            break;
        }
        // Get parent PID from /proc/{pid}/stat
        let stat_path = format!("/proc/{}/stat", pid);
        if let Ok(stat) = std::fs::read_to_string(&stat_path) {
            // Format: "pid (comm) state ppid ..."
            // Find the closing ')' to skip the comm field (which may contain spaces)
            if let Some(after_comm) = stat.rfind(')') {
                let remainder = &stat[after_comm + 1..];
                let fields: Vec<&str> = remainder.split_whitespace().collect();
                // fields[0] = state, fields[1] = ppid
                if let Some(ppid_str) = fields.get(1)
                    && let Ok(ppid) = ppid_str.parse::<u32>()
                {
                    pid = ppid;
                    continue;
                }
            }
        }
        break;
    }
    false
}

pub fn detect() -> Option<String> {
    let ssh = std::env::var("SSH_CONNECTION").ok();

    #[cfg(target_os = "linux")]
    let mosh = check_mosh_in_tree();
    #[cfg(not(target_os = "linux"))]
    let mosh = false;

    detect_from_env(ssh.as_deref(), mosh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ssh() {
        assert_eq!(
            detect_from_env(Some("10.0.0.1 54321 10.0.0.2 22"), false),
            Some("ssh".to_string())
        );
    }

    #[test]
    fn does_not_leak_ip() {
        let result = detect_from_env(Some("10.0.0.1 54321 10.0.0.2 22"), false);
        // Should return "ssh", not the IP addresses
        assert_eq!(result, Some("ssh".to_string()));
    }

    #[test]
    fn detects_mosh() {
        assert_eq!(detect_from_env(None, true), Some("mosh".to_string()));
    }

    #[test]
    fn ssh_takes_priority_over_mosh() {
        // If SSH_CONNECTION is set, report ssh even if mosh is in tree
        assert_eq!(
            detect_from_env(Some("1.2.3.4 5678 5.6.7.8 22"), true),
            Some("ssh".to_string())
        );
    }

    #[test]
    fn none_when_local() {
        assert_eq!(detect_from_env(None, false), None);
    }

    #[test]
    fn none_when_empty_ssh_connection() {
        assert_eq!(detect_from_env(Some(""), false), None);
    }
}
