// Phase 3: EnvContext and detect_all()

use serde::Serialize;

pub mod container;
pub mod git;
pub mod multiplexer;
pub mod os;
pub mod packages;
pub mod remote;
pub mod shell;
pub mod tty;

#[derive(Serialize, Debug)]
pub struct EnvContext {
    pub os: String,
    pub os_version: Option<String>,
    pub os_family: Option<String>,
    pub kernel: Option<String>,
    pub shell: String,
    pub interactive: bool,
    pub multiplexer: Option<String>,
    pub remote: Option<String>,
    pub container: Option<String>,
    pub package_managers: Vec<String>,
    pub cwd: Option<String>,
    pub git_repo: bool,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

/// Detect all environment context. This function is infallible.
pub fn detect_all() -> EnvContext {
    let os_info = os::detect();
    let shell_name = shell::detect();
    let interactive = tty::stdin_is_terminal();
    let mux = multiplexer::detect();
    let remote = remote::detect();
    let container = container::detect();
    let pkg_managers = packages::detect();
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().to_string());
    let git_info = git::detect();

    EnvContext {
        os: os_info.os,
        os_version: os_info.os_version,
        os_family: os_info.os_family,
        kernel: os_info.kernel,
        shell: shell_name,
        interactive,
        multiplexer: mux,
        remote,
        container,
        package_managers: pkg_managers,
        cwd,
        git_repo: git_info.git_repo,
        git_branch: git_info.git_branch,
        git_dirty: git_info.git_dirty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_all_produces_valid_context() {
        let ctx = detect_all();

        // OS must be non-empty
        assert!(!ctx.os.is_empty());

        // Shell must be non-empty
        assert!(!ctx.shell.is_empty());

        // cwd should exist (we're running tests, so a cwd must exist)
        assert!(ctx.cwd.is_some());

        // We're running inside a git repo
        assert!(ctx.git_repo);
        assert!(ctx.git_branch.is_some());

        // package_managers should be a Vec (even if empty)
        // Just verify it's accessible
        let _ = ctx.package_managers.len();
    }

    #[test]
    fn env_context_serializes_to_json() {
        let ctx = detect_all();
        let json = serde_json::to_string(&ctx).expect("should serialize to JSON");
        assert!(json.contains("\"os\""));
        assert!(json.contains("\"shell\""));
        assert!(json.contains("\"git_repo\""));
        assert!(json.contains("\"package_managers\""));
    }

    #[test]
    fn env_context_debug_fmt() {
        let ctx = detect_all();
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("EnvContext"));
    }
}
