// Phase 3: tmux/screen/zellij detection

/// Detect which terminal multiplexer is active based on env var values.
/// Takes the values of $TMUX, $STY, and $ZELLIJ as parameters.
pub fn detect_from_env(
    tmux: Option<&str>,
    sty: Option<&str>,
    zellij: Option<&str>,
) -> Option<String> {
    if tmux.is_some_and(|v| !v.is_empty()) {
        return Some("tmux".to_string());
    }
    if sty.is_some_and(|v| !v.is_empty()) {
        return Some("screen".to_string());
    }
    if zellij.is_some_and(|v| !v.is_empty()) {
        return Some("zellij".to_string());
    }
    None
}

/// Detect multiplexer from the real environment.
pub fn detect() -> Option<String> {
    detect_from_env(
        std::env::var("TMUX").ok().as_deref(),
        std::env::var("STY").ok().as_deref(),
        std::env::var("ZELLIJ").ok().as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tmux() {
        assert_eq!(
            detect_from_env(Some("/tmp/tmux-1000/default,12345,0"), None, None),
            Some("tmux".to_string())
        );
    }

    #[test]
    fn detects_screen() {
        assert_eq!(
            detect_from_env(None, Some("12345.pts-0.host"), None),
            Some("screen".to_string())
        );
    }

    #[test]
    fn detects_zellij() {
        assert_eq!(
            detect_from_env(None, None, Some("0")),
            Some("zellij".to_string())
        );
    }

    #[test]
    fn tmux_takes_priority() {
        assert_eq!(
            detect_from_env(Some("tmux"), Some("screen"), Some("zellij")),
            Some("tmux".to_string())
        );
    }

    #[test]
    fn none_when_all_absent() {
        assert_eq!(detect_from_env(None, None, None), None);
    }

    #[test]
    fn none_when_all_empty() {
        assert_eq!(detect_from_env(Some(""), Some(""), Some("")), None);
    }
}
