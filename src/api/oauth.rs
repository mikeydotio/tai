use serde::Deserialize;

#[derive(Deserialize)]
struct CredentialsFile {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: Option<OAuthEntry>,
}

#[derive(Deserialize)]
struct OAuthEntry {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: u64,
}

pub struct ClaudeOAuthCredentials {
    pub access_token: String,
    #[allow(dead_code)] // Exposed for future use (e.g., token refresh)
    pub expires_at: u64,
}

/// Discover Claude Code OAuth credentials from ~/.claude/.credentials.json.
/// Returns None if file is missing, expired, or unparseable.
pub fn discover_claude_oauth() -> Option<ClaudeOAuthCredentials> {
    let path = credentials_path()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let creds: CredentialsFile = serde_json::from_str(&contents).ok()?;
    let oauth = creds.claude_ai_oauth?;

    // Check expiration (expires_at is in milliseconds since epoch)
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;

    if oauth.expires_at <= now_ms {
        return None;
    }

    Some(ClaudeOAuthCredentials {
        access_token: oauth.access_token,
        expires_at: oauth.expires_at,
    })
}

fn credentials_path() -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::PathBuf::from(home)
            .join(".claude")
            .join(".credentials.json"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_credentials() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "sk-ant-oat01-test-token",
                "refreshToken": "sk-ant-ort01-test-refresh",
                "expiresAt": 9999999999999,
                "scopes": ["user:inference"],
                "subscriptionType": "max"
            }
        }"#;
        let creds: CredentialsFile = serde_json::from_str(json).unwrap();
        let oauth = creds.claude_ai_oauth.unwrap();
        assert_eq!(oauth.access_token, "sk-ant-oat01-test-token");
        assert_eq!(oauth.expires_at, 9999999999999);
    }

    #[test]
    fn parse_missing_oauth_field() {
        let json = r#"{}"#;
        let creds: CredentialsFile = serde_json::from_str(json).unwrap();
        assert!(creds.claude_ai_oauth.is_none());
    }

    #[test]
    fn parse_malformed_json() {
        let result = serde_json::from_str::<CredentialsFile>("not json");
        assert!(result.is_err());
    }

    #[test]
    fn expired_token_detected() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "token",
                "refreshToken": "refresh",
                "expiresAt": 1000
            }
        }"#;
        let creds: CredentialsFile = serde_json::from_str(json).unwrap();
        let oauth = creds.claude_ai_oauth.unwrap();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(oauth.expires_at <= now_ms);
    }

    #[test]
    fn future_token_is_valid() {
        let json = r#"{
            "claudeAiOauth": {
                "accessToken": "token",
                "refreshToken": "refresh",
                "expiresAt": 9999999999999
            }
        }"#;
        let creds: CredentialsFile = serde_json::from_str(json).unwrap();
        let oauth = creds.claude_ai_oauth.unwrap();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(oauth.expires_at > now_ms);
    }
}
