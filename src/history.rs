// Phase 8: Local history — append-only SQLite log of every tai invocation.
//
// Design principles:
// - `record()` is infallible from the caller's perspective: errors are logged
//   to stderr and swallowed so that a corrupt or missing DB never blocks
//   normal operation.
// - The DB lives at $XDG_DATA_HOME/tai/history.db (or ~/.local/share/tai/history.db).
// - `show()` prints recent history to stderr with colored formatting.

use std::path::PathBuf;

use owo_colors::OwoColorize;
use rusqlite::{Connection, params};

/// Derive `user_choice` from action mode and outcome.
///
/// | mode    | outcome          | user_choice |
/// |---------|------------------|-------------|
/// | propose | Ok(_)            | "accepted"  |
/// | propose | Err(UserDeclined)| "declined"  |
/// | act     | _                | "accepted"  |
/// | inform  | _                | NULL        |
pub fn user_choice(action: crate::cli::ActionMode, declined: bool) -> Option<&'static str> {
    match action {
        crate::cli::ActionMode::Propose => {
            if declined {
                Some("declined")
            } else {
                Some("accepted")
            }
        }
        crate::cli::ActionMode::Act => Some("accepted"),
        crate::cli::ActionMode::Inform => None,
    }
}

/// Record a single invocation to the history database.
///
/// This function is intentionally infallible: any error is printed to stderr
/// and then discarded, so the caller's control flow is never disrupted.
pub fn record(
    prompt: &str,
    command: Option<&str>,
    action: &str,
    model: &str,
    provider: &str,
    exit_code: i32,
    user_choice: Option<&str>,
) {
    if let Err(e) = record_inner(
        prompt,
        command,
        action,
        model,
        provider,
        exit_code,
        user_choice,
    ) {
        eprintln!("tai: history: {}", e);
    }
}

fn record_inner(
    prompt: &str,
    command: Option<&str>,
    action: &str,
    model: &str,
    provider: &str,
    exit_code: i32,
    user_choice: Option<&str>,
) -> Result<(), rusqlite::Error> {
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO history (prompt, command, action, model, provider, exit_code, user_choice)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            prompt,
            command,
            action,
            model,
            provider,
            exit_code,
            user_choice
        ],
    )?;
    Ok(())
}

/// Print recent history entries to stderr.
///
/// Returns `Ok(0)` on success, or an error on database failure.
pub fn show(limit: usize) -> Result<i32, crate::error::TaiError> {
    let db_path = db_path().ok_or_else(|| {
        crate::error::TaiError::Config("cannot determine history database path".into())
    })?;

    if !db_path.exists() {
        eprintln!("tai: no history yet");
        return Ok(0);
    }

    let conn = Connection::open(&db_path)
        .map_err(|e| crate::error::TaiError::Config(format!("history db: {}", e)))?;

    let mut stmt = conn
        .prepare(
            "SELECT timestamp, prompt, command, action, exit_code, user_choice
             FROM history ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| crate::error::TaiError::Config(format!("history query: {}", e)))?;

    let color = use_color();

    let rows = stmt
        .query_map(params![limit], |row| {
            Ok(HistoryRow {
                timestamp: row.get(0)?,
                prompt: row.get(1)?,
                command: row.get(2)?,
                action: row.get(3)?,
                exit_code: row.get(4)?,
                user_choice: row.get(5)?,
            })
        })
        .map_err(|e| crate::error::TaiError::Config(format!("history query: {}", e)))?;

    let mut entries: Vec<HistoryRow> = Vec::new();
    for row in rows {
        match row {
            Ok(r) => entries.push(r),
            Err(e) => {
                eprintln!("tai: history: skipping corrupt row: {}", e);
            }
        }
    }

    if entries.is_empty() {
        eprintln!("tai: no history yet");
        return Ok(0);
    }

    // Print in chronological order (oldest first) since we queried DESC
    for entry in entries.iter().rev() {
        print_entry(entry, color);
    }

    Ok(0)
}

struct HistoryRow {
    timestamp: String,
    prompt: String,
    command: Option<String>,
    action: String,
    exit_code: i32,
    user_choice: Option<String>,
}

fn print_entry(entry: &HistoryRow, color: bool) {
    if color {
        eprintln!(
            "{} {} {}",
            entry.timestamp.dimmed(),
            format_action_badge(&entry.action, &entry.user_choice, entry.exit_code, true),
            entry.prompt.bold(),
        );
        if let Some(ref cmd) = entry.command {
            eprintln!("  {} {}", ">".dimmed(), cmd.green());
        }
    } else {
        eprintln!(
            "{} {} {}",
            entry.timestamp,
            format_action_badge(&entry.action, &entry.user_choice, entry.exit_code, false),
            entry.prompt,
        );
        if let Some(ref cmd) = entry.command {
            eprintln!("  > {}", cmd);
        }
    }
}

fn format_action_badge(
    action: &str,
    user_choice: &Option<String>,
    exit_code: i32,
    color: bool,
) -> String {
    let label = match (action, user_choice.as_deref()) {
        ("propose", Some("accepted")) => "[proposed -> ran]",
        ("propose", Some("declined")) => "[proposed -> declined]",
        ("act", _) => "[acted]",
        ("inform", _) => "[informed]",
        _ => "[unknown]",
    };

    if !color {
        return label.to_string();
    }

    // Color the badge based on outcome
    match exit_code {
        0 => format!("{}", label.green()),
        _ => format!("{}", label.yellow()),
    }
}

/// Check whether stderr supports color output.
fn use_color() -> bool {
    supports_color::on(supports_color::Stream::Stderr).is_some()
}

// ---------------------------------------------------------------------------
// Database helpers
// ---------------------------------------------------------------------------

/// Return the path to history.db under XDG_DATA_HOME.
fn db_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return Some(PathBuf::from(xdg).join("tai").join("history.db"));
    }
    if let Ok(home) = std::env::var("HOME") {
        return Some(
            PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("tai")
                .join("history.db"),
        );
    }
    None
}

/// Open (or create) the history database and ensure the schema exists.
fn open_db() -> Result<Connection, rusqlite::Error> {
    let path = db_path().ok_or_else(|| {
        rusqlite::Error::InvalidPath("cannot determine XDG_DATA_HOME or HOME".into())
    })?;

    // Ensure parent directories exist
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')),
            prompt      TEXT NOT NULL,
            command     TEXT,
            action      TEXT NOT NULL,
            model       TEXT NOT NULL,
            provider    TEXT NOT NULL,
            exit_code   INTEGER NOT NULL,
            user_choice TEXT
        );",
    )?;
    Ok(conn)
}

/// Open a database at a specific path (for testing).
#[cfg(test)]
fn open_db_at(path: &std::path::Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')),
            prompt      TEXT NOT NULL,
            command     TEXT,
            action      TEXT NOT NULL,
            model       TEXT NOT NULL,
            provider    TEXT NOT NULL,
            exit_code   INTEGER NOT NULL,
            user_choice TEXT
        );",
    )?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: insert a record directly via a connection at a given path.
    fn insert_at(
        conn: &Connection,
        prompt: &str,
        command: Option<&str>,
        action: &str,
        model: &str,
        provider: &str,
        exit_code: i32,
        user_choice: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO history (prompt, command, action, model, provider, exit_code, user_choice)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                prompt,
                command,
                action,
                model,
                provider,
                exit_code,
                user_choice
            ],
        )
        .expect("insert should succeed");
    }

    #[test]
    fn schema_creation_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let _conn1 = open_db_at(&db).unwrap();
        let _conn2 = open_db_at(&db).unwrap(); // second open should not fail
    }

    #[test]
    fn insert_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let conn = open_db_at(&db).unwrap();

        insert_at(
            &conn,
            "list files",
            Some("ls -la"),
            "propose",
            "claude-sonnet-4",
            "anthropic",
            0,
            Some("accepted"),
        );

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        let prompt: String = conn
            .query_row("SELECT prompt FROM history WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(prompt, "list files");
    }

    #[test]
    fn null_command_stored_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let conn = open_db_at(&db).unwrap();

        insert_at(
            &conn,
            "what is rust",
            None,
            "inform",
            "gpt-4o",
            "openai",
            0,
            None,
        );

        let command: Option<String> = conn
            .query_row("SELECT command FROM history WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(command.is_none());

        let choice: Option<String> = conn
            .query_row("SELECT user_choice FROM history WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(choice.is_none());
    }

    #[test]
    fn user_choice_propose_accepted() {
        let choice = user_choice(crate::cli::ActionMode::Propose, false);
        assert_eq!(choice, Some("accepted"));
    }

    #[test]
    fn user_choice_propose_declined() {
        let choice = user_choice(crate::cli::ActionMode::Propose, true);
        assert_eq!(choice, Some("declined"));
    }

    #[test]
    fn user_choice_act_always_accepted() {
        let choice = user_choice(crate::cli::ActionMode::Act, false);
        assert_eq!(choice, Some("accepted"));
    }

    #[test]
    fn user_choice_inform_always_none() {
        let choice = user_choice(crate::cli::ActionMode::Inform, false);
        assert_eq!(choice, None);
    }

    #[test]
    fn multiple_rows_ordered_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let conn = open_db_at(&db).unwrap();

        insert_at(
            &conn,
            "first",
            Some("cmd1"),
            "act",
            "m",
            "p",
            0,
            Some("accepted"),
        );
        insert_at(
            &conn,
            "second",
            Some("cmd2"),
            "act",
            "m",
            "p",
            0,
            Some("accepted"),
        );
        insert_at(
            &conn,
            "third",
            Some("cmd3"),
            "propose",
            "m",
            "p",
            1,
            Some("declined"),
        );

        let mut stmt = conn
            .prepare("SELECT prompt FROM history ORDER BY id ASC")
            .unwrap();
        let prompts: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();

        assert_eq!(prompts, vec!["first", "second", "third"]);
    }

    #[test]
    fn timestamp_auto_populated() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("history.db");
        let conn = open_db_at(&db).unwrap();

        insert_at(
            &conn,
            "test",
            Some("echo hi"),
            "act",
            "m",
            "p",
            0,
            Some("accepted"),
        );

        let ts: String = conn
            .query_row("SELECT timestamp FROM history WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();

        // Timestamp should look like an ISO-8601 local time: YYYY-MM-DDTHH:MM:SS
        assert!(ts.len() >= 19, "timestamp too short: {}", ts);
        assert!(ts.contains('T'), "timestamp missing T separator: {}", ts);
    }

    #[test]
    fn format_action_badge_plain() {
        let badge = format_action_badge("propose", &Some("accepted".into()), 0, false);
        assert_eq!(badge, "[proposed -> ran]");

        let badge = format_action_badge("propose", &Some("declined".into()), 74, false);
        assert_eq!(badge, "[proposed -> declined]");

        let badge = format_action_badge("act", &None, 0, false);
        assert_eq!(badge, "[acted]");

        let badge = format_action_badge("inform", &None, 0, false);
        assert_eq!(badge, "[informed]");
    }

    #[test]
    fn record_via_env_override() {
        // Use a tempdir as XDG_DATA_HOME to test the `record` function end-to-end.
        let dir = tempfile::tempdir().unwrap();

        // SAFETY: This test is run single-threaded in its own process; no
        // other thread reads XDG_DATA_HOME concurrently.
        let prev = std::env::var("XDG_DATA_HOME").ok();
        unsafe { std::env::set_var("XDG_DATA_HOME", dir.path()) };

        record(
            "deploy the app",
            Some("kubectl apply -f deploy.yaml"),
            "act",
            "claude-sonnet-4",
            "anthropic",
            0,
            Some("accepted"),
        );

        // Verify it landed in the DB
        let db = dir.path().join("tai").join("history.db");
        assert!(db.exists(), "history.db should have been created");

        let conn = Connection::open(&db).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // Restore
        unsafe {
            match prev {
                Some(val) => std::env::set_var("XDG_DATA_HOME", val),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }
}
