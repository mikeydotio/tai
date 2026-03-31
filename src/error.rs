#[derive(Debug, thiserror::Error)]
pub enum TaiError {
    #[error("no prompt provided")]
    NoPrompt,

    #[error("config error: {0}")]
    Config(String),

    #[error("{0} not found in PATH")]
    CliNotFound(String),

    #[error("user declined")]
    UserDeclined,

    #[error("API request failed: {0}")]
    ApiRequest(String),

    #[error("failed to parse response: {0}")]
    ResponseParse(String),

    #[error("{0}")]
    Exec(#[from] std::io::Error),
}

impl TaiError {
    pub fn exit_code(&self) -> i32 {
        match self {
            TaiError::NoPrompt => 64,
            TaiError::Config(_) => 65,
            TaiError::CliNotFound(_) => 69,
            TaiError::UserDeclined => 74,
            TaiError::ApiRequest(_) => 76,
            TaiError::ResponseParse(_) => 77,
            TaiError::Exec(_) => 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_prompt_exit_code() {
        assert_eq!(TaiError::NoPrompt.exit_code(), 64);
    }

    #[test]
    fn config_exit_code() {
        assert_eq!(TaiError::Config("bad".into()).exit_code(), 65);
    }

    #[test]
    fn cli_not_found_exit_code() {
        assert_eq!(TaiError::CliNotFound("claude".into()).exit_code(), 69);
    }

    #[test]
    fn user_declined_exit_code() {
        assert_eq!(TaiError::UserDeclined.exit_code(), 74);
    }

    #[test]
    fn api_request_exit_code() {
        assert_eq!(TaiError::ApiRequest("err".into()).exit_code(), 76);
    }

    #[test]
    fn response_parse_exit_code() {
        assert_eq!(TaiError::ResponseParse("err".into()).exit_code(), 77);
    }

    #[test]
    fn exec_exit_code() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        assert_eq!(TaiError::Exec(io_err).exit_code(), 1);
    }

    #[test]
    fn display_messages() {
        assert_eq!(TaiError::NoPrompt.to_string(), "no prompt provided");
        assert_eq!(
            TaiError::Config("bad toml".into()).to_string(),
            "config error: bad toml"
        );
        assert_eq!(
            TaiError::CliNotFound("claude".into()).to_string(),
            "claude not found in PATH"
        );
    }
}
