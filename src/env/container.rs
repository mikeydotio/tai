// Phase 3: Container runtime detection (Docker/Podman/LXC/K8s)

/// Detect container from a cascade of checks.
/// Parameters represent the results of each detection layer:
/// - `dockerenv_exists`: whether `/.dockerenv` exists
/// - `containerenv_exists`: whether `/run/.containerenv` exists
/// - `container_env`: value of `$container` env var
/// - `k8s_host_env`: value of `$KUBERNETES_SERVICE_HOST`
/// - `mountinfo_content`: content of `/proc/self/mountinfo`
pub fn detect_from_inputs(
    dockerenv_exists: bool,
    containerenv_exists: bool,
    container_env: Option<&str>,
    k8s_host_env: Option<&str>,
    mountinfo_content: Option<&str>,
) -> Option<String> {
    if dockerenv_exists {
        return Some("docker".to_string());
    }
    if containerenv_exists {
        return Some("podman".to_string());
    }
    if let Some(val) = container_env
        && !val.is_empty()
    {
        return Some(val.to_string());
    }
    if k8s_host_env.is_some_and(|v| !v.is_empty()) {
        return Some("kubernetes".to_string());
    }
    if let Some(content) = mountinfo_content {
        if content.contains("kubepods") {
            return Some("kubernetes".to_string());
        }
        if content.contains("docker") {
            return Some("docker".to_string());
        }
        if content.contains("containerd") {
            return Some("containerd".to_string());
        }
        if content.contains("/lxc/") {
            return Some("lxc".to_string());
        }
    }
    None
}

pub fn detect() -> Option<String> {
    if !cfg!(target_os = "linux") {
        return None;
    }

    let dockerenv_exists = std::path::Path::new("/.dockerenv").exists();
    let containerenv_exists = std::path::Path::new("/run/.containerenv").exists();
    let container_env = std::env::var("container").ok();
    let k8s_host_env = std::env::var("KUBERNETES_SERVICE_HOST").ok();
    let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").ok();

    detect_from_inputs(
        dockerenv_exists,
        containerenv_exists,
        container_env.as_deref(),
        k8s_host_env.as_deref(),
        mountinfo.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_docker_from_dockerenv() {
        assert_eq!(
            detect_from_inputs(true, false, None, None, None),
            Some("docker".to_string())
        );
    }

    #[test]
    fn detects_podman_from_containerenv() {
        assert_eq!(
            detect_from_inputs(false, true, None, None, None),
            Some("podman".to_string())
        );
    }

    #[test]
    fn detects_container_env_var() {
        assert_eq!(
            detect_from_inputs(false, false, Some("systemd-nspawn"), None, None),
            Some("systemd-nspawn".to_string())
        );
    }

    #[test]
    fn detects_kubernetes_from_env() {
        assert_eq!(
            detect_from_inputs(false, false, None, Some("10.0.0.1"), None),
            Some("kubernetes".to_string())
        );
    }

    #[test]
    fn detects_docker_from_mountinfo() {
        let mountinfo = "1234 100 8:1 / / rw - overlay overlay rw,lowerdir=/var/lib/docker/overlay2/abc";
        assert_eq!(
            detect_from_inputs(false, false, None, None, Some(mountinfo)),
            Some("docker".to_string())
        );
    }

    #[test]
    fn detects_kubernetes_from_mountinfo() {
        let mountinfo = "1234 100 8:1 / / rw - overlay overlay rw,lowerdir=/sys/fs/cgroup/kubepods/abc";
        assert_eq!(
            detect_from_inputs(false, false, None, None, Some(mountinfo)),
            Some("kubernetes".to_string())
        );
    }

    #[test]
    fn detects_lxc_from_mountinfo() {
        let mountinfo = "1234 100 8:1 /lxc/container1 / rw - overlay overlay rw";
        assert_eq!(
            detect_from_inputs(false, false, None, None, Some(mountinfo)),
            Some("lxc".to_string())
        );
    }

    #[test]
    fn dockerenv_takes_priority() {
        assert_eq!(
            detect_from_inputs(true, true, Some("lxc"), Some("10.0.0.1"), None),
            Some("docker".to_string())
        );
    }

    #[test]
    fn none_when_not_in_container() {
        assert_eq!(
            detect_from_inputs(false, false, None, None, Some("normal mountinfo here")),
            None
        );
    }

    #[test]
    fn none_when_all_absent() {
        assert_eq!(
            detect_from_inputs(false, false, None, None, None),
            None
        );
    }

    #[test]
    fn empty_container_env_ignored() {
        assert_eq!(
            detect_from_inputs(false, false, Some(""), None, None),
            None
        );
    }
}
