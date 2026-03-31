// Phase 3: OS/distro detection

use std::process::Command;

pub struct OsInfo {
    pub os: String,
    pub os_version: Option<String>,
    pub os_family: Option<String>,
    pub kernel: Option<String>,
}

/// Parse /etc/os-release content to extract ID, VERSION_ID, and ID_LIKE.
pub fn parse_os_release(content: &str) -> (String, Option<String>, Option<String>) {
    let mut id = None;
    let mut version_id = None;
    let mut id_like = None;

    for line in content.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            let value = value.trim_matches('"').trim_matches('\'');
            match key {
                "ID" => id = Some(value.to_string()),
                "VERSION_ID" => version_id = Some(value.to_string()),
                "ID_LIKE" => id_like = Some(value.to_string()),
                _ => {}
            }
        }
    }

    (
        id.unwrap_or_else(|| "linux".to_string()),
        version_id,
        id_like,
    )
}

/// Extract kernel version from /proc/version content.
pub fn parse_proc_version(content: &str) -> Option<String> {
    // Format: "Linux version 5.15.0-generic ..."
    let parts: Vec<&str> = content.split_whitespace().collect();
    if parts.len() >= 3 && parts[0] == "Linux" && parts[1] == "version" {
        Some(parts[2].to_string())
    } else {
        None
    }
}

pub fn detect() -> OsInfo {
    if cfg!(target_os = "linux") {
        detect_linux()
    } else if cfg!(target_os = "macos") {
        detect_macos()
    } else {
        OsInfo {
            os: std::env::consts::OS.to_string(),
            os_version: None,
            os_family: None,
            kernel: None,
        }
    }
}

fn detect_linux() -> OsInfo {
    let (os, os_version, os_family) = match std::fs::read_to_string("/etc/os-release") {
        Ok(content) => parse_os_release(&content),
        Err(_) => ("linux".to_string(), None, None),
    };

    let kernel = std::fs::read_to_string("/proc/version")
        .ok()
        .and_then(|content| parse_proc_version(&content))
        .or_else(|| {
            Command::new("uname")
                .arg("-r")
                .output()
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                })
        });

    OsInfo {
        os,
        os_version,
        os_family,
        kernel,
    }
}

fn detect_macos() -> OsInfo {
    let os_version = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    let kernel = Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    OsInfo {
        os: "macos".to_string(),
        os_version,
        os_family: None,
        kernel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ubuntu_os_release() {
        let content = r#"NAME="Ubuntu"
VERSION="22.04.3 LTS (Jammy Jellyfish)"
ID=ubuntu
ID_LIKE=debian
VERSION_ID="22.04"
"#;
        let (os, version, family) = parse_os_release(content);
        assert_eq!(os, "ubuntu");
        assert_eq!(version, Some("22.04".to_string()));
        assert_eq!(family, Some("debian".to_string()));
    }

    #[test]
    fn parse_arch_os_release() {
        let content = r#"NAME="Arch Linux"
ID=arch
ID_LIKE=archlinux
"#;
        let (os, version, family) = parse_os_release(content);
        assert_eq!(os, "arch");
        assert_eq!(version, None);
        assert_eq!(family, Some("archlinux".to_string()));
    }

    #[test]
    fn parse_alpine_os_release() {
        let content = r#"NAME="Alpine Linux"
ID=alpine
VERSION_ID=3.18.4
"#;
        let (os, version, family) = parse_os_release(content);
        assert_eq!(os, "alpine");
        assert_eq!(version, Some("3.18.4".to_string()));
        assert_eq!(family, None);
    }

    #[test]
    fn parse_nixos_os_release() {
        let content = r#"NAME=NixOS
ID=nixos
VERSION="23.11 (Tapir)"
VERSION_ID="23.11"
ID_LIKE="nixos"
"#;
        let (os, version, family) = parse_os_release(content);
        assert_eq!(os, "nixos");
        assert_eq!(version, Some("23.11".to_string()));
        assert_eq!(family, Some("nixos".to_string()));
    }

    #[test]
    fn parse_empty_os_release() {
        let (os, version, family) = parse_os_release("");
        assert_eq!(os, "linux");
        assert_eq!(version, None);
        assert_eq!(family, None);
    }

    #[test]
    fn parse_proc_version_typical() {
        let content = "Linux version 5.15.0-91-generic (buildd@bos03-amd64-016) (gcc (Ubuntu 11.4.0-1ubuntu1~22.04) 11.4.0, GNU ld (GNU Binutils for Ubuntu) 2.38) #101-Ubuntu SMP";
        let kernel = parse_proc_version(content);
        assert_eq!(kernel, Some("5.15.0-91-generic".to_string()));
    }

    #[test]
    fn parse_proc_version_empty() {
        assert_eq!(parse_proc_version(""), None);
    }

    #[test]
    fn parse_proc_version_garbage() {
        assert_eq!(parse_proc_version("not a valid proc version"), None);
    }

    #[test]
    fn detect_returns_something() {
        let info = detect();
        assert!(!info.os.is_empty());
    }
}
