//! Convert git remote URLs into browser-openable HTTPS links.

/// Strip a trailing `.git` suffix when present.
fn strip_git_suffix(path: &str) -> &str {
    path.strip_suffix(".git").unwrap_or(path)
}

/// Best-effort conversion of a git remote URL to an HTTPS (or HTTP) browse URL.
///
/// Handles:
/// - `https://` / `http://`
/// - `git@host:path` (GitHub / GitLab / Bitbucket SSH)
/// - `ssh://[user@]host[:port]/path`
/// - `git://host/path`
pub fn remote_browse_url(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }

    if let Some(rest) = s.strip_prefix("https://") {
        return Some(format!("https://{}", strip_git_suffix(rest.trim_end_matches('/'))));
    }
    if let Some(rest) = s.strip_prefix("http://") {
        return Some(format!("http://{}", strip_git_suffix(rest.trim_end_matches('/'))));
    }

    if let Some(rest) = s.strip_prefix("git://") {
        return Some(format!("https://{}", strip_git_suffix(rest.trim_end_matches('/'))));
    }

    if let Some(rest) = s.strip_prefix("ssh://") {
        return ssh_authority_to_https(rest);
    }

    // scp-like: git@github.com:user/repo.git
    if let Some((userhost, path)) = s.split_once(':') {
        if !userhost.contains("://") && (userhost.contains('@') || !userhost.contains('/')) {
            let host = userhost.rsplit('@').next().unwrap_or(userhost);
            if host.is_empty() || path.is_empty() {
                return None;
            }
            let path = strip_git_suffix(path.trim_start_matches('/'));
            return Some(format!("https://{host}/{path}"));
        }
    }

    None
}

fn ssh_authority_to_https(rest: &str) -> Option<String> {
    // [user@]host[:port]/path
    let rest = rest.split_once('@').map(|(_, host)| host).unwrap_or(rest);
    let (host_port, path) = rest.split_once('/')?;
    if path.is_empty() {
        return None;
    }
    let host = host_port.split(':').next().unwrap_or(host_port);
    if host.is_empty() {
        return None;
    }
    Some(format!(
        "https://{host}/{}",
        strip_git_suffix(path.trim_end_matches('/'))
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_ssh_scp() {
        assert_eq!(
            remote_browse_url("git@github.com:owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn parses_github_ssh_url() {
        assert_eq!(
            remote_browse_url("ssh://git@github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn parses_https() {
        assert_eq!(
            remote_browse_url("https://github.com/owner/repo.git").as_deref(),
            Some("https://github.com/owner/repo")
        );
    }

    #[test]
    fn parses_gitlab_ssh() {
        assert_eq!(
            remote_browse_url("git@gitlab.com:group/sub/repo.git").as_deref(),
            Some("https://gitlab.com/group/sub/repo")
        );
    }
}
