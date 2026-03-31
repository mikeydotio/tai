// Phase 3: Package manager detection via PATH scan

const KNOWN_MANAGERS: &[&str] = &[
    "apt", "dnf", "yum", "pacman", "zypper", "apk", "emerge", "brew", "port", "nix", "snap",
    "flatpak",
];

/// Given a list of package manager names that were found, return them.
/// This is the testable core: the caller decides how to check existence.
pub fn filter_found(candidates: &[&str], checker: impl Fn(&str) -> bool) -> Vec<String> {
    candidates
        .iter()
        .filter(|name| checker(name))
        .map(|name| name.to_string())
        .collect()
}

/// Detect package managers by checking if they exist in PATH.
pub fn detect() -> Vec<String> {
    filter_found(KNOWN_MANAGERS, |name| which::which(name).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_found_all_present() {
        let result = filter_found(&["apt", "brew", "nix"], |_| true);
        assert_eq!(result, vec!["apt", "brew", "nix"]);
    }

    #[test]
    fn filter_found_none_present() {
        let result = filter_found(&["apt", "brew", "nix"], |_| false);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_found_some_present() {
        let result = filter_found(&["apt", "brew", "pacman"], |name| name == "brew");
        assert_eq!(result, vec!["brew"]);
    }

    #[test]
    fn detect_returns_vec() {
        // On any system, this should return a Vec (possibly empty)
        let managers = detect();
        // All returned names should be from the known list
        for mgr in &managers {
            assert!(KNOWN_MANAGERS.contains(&mgr.as_str()));
        }
    }

    #[test]
    fn known_managers_is_not_empty() {
        assert!(!KNOWN_MANAGERS.is_empty());
    }
}
