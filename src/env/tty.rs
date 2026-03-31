use std::io::IsTerminal;

pub fn stdin_is_terminal() -> bool {
    std::io::stdin().is_terminal()
}

#[allow(dead_code)]
pub fn stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}
