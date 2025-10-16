use crate::pager_overlay::Overlay;
use codex_ansi_escape::ansi_escape_line;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;

/// Session metadata
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    #[allow(dead_code)]
    pub path: PathBuf,
    #[allow(dead_code)]
    pub cwd: String,
    pub age: String,
    #[allow(dead_code)]
    pub mtime: u64,
}

/// Get current working directory
fn get_cwd() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))
}

/// Get sessions directory
fn get_sessions_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME").map_err(|e| format!("Failed to get HOME: {}", e))?;
    Ok(PathBuf::from(home).join(".codex/sessions"))
}

/// Extract session metadata from .jsonl file
fn extract_session_meta(path: &PathBuf) -> Result<(String, String), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);
    let mut lines = reader.lines();

    if let Some(Ok(first_line)) = lines.next() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&first_line) {
            // Try to extract session ID and CWD from session_meta
            if let Some(payload) = json.get("payload") {
                let id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());

                let cwd = payload
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                return Ok((id, cwd));
            }
        }
    }

    // Fallback: use filename as ID
    let id = path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    Ok((id, String::new()))
}

/// Format relative time
fn format_relative_time(mtime: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let diff = now.saturating_sub(mtime);
    let s = diff;

    if s < 60 {
        format!("{}s ago", s)
    } else if s < 3600 {
        format!("{}m ago", s / 60)
    } else if s < 86400 {
        format!("{}h ago", s / 3600)
    } else if s < 604800 {
        format!("{}d ago", s / 86400)
    } else if s < 2592000 {
        format!("{}w ago", s / 604800)
    } else if s < 31536000 {
        format!("{}mo ago", s / 2592000)
    } else {
        format!("{}y ago", s / 31536000)
    }
}

/// Get sessions in current working directory
pub fn get_cwd_sessions() -> Result<Vec<SessionInfo>, String> {
    let cwd = get_cwd()?;
    let sessions_dir = get_sessions_dir()?;

    let cwd_str = cwd.to_string_lossy().to_string();

    // Recursively find all .jsonl files
    let mut sessions = Vec::new();

    fn find_sessions(
        dir: &PathBuf,
        cwd: &str,
        sessions: &mut Vec<SessionInfo>,
        max_depth: u32,
    ) -> Result<(), String> {
        if max_depth == 0 {
            return Ok(());
        }

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "jsonl") {
                        if let Ok((id, session_cwd)) = extract_session_meta(&path) {
                            // Filter by current working directory
                            if session_cwd.is_empty() || session_cwd == cwd {
                                let mtime = entry
                                    .metadata()
                                    .ok()
                                    .and_then(|m| m.modified().ok())
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);

                                let age = format_relative_time(mtime);

                                sessions.push(SessionInfo {
                                    id,
                                    path: path.clone(),
                                    cwd: session_cwd,
                                    age,
                                    mtime,
                                });
                            }
                        }
                    } else if path.is_dir() {
                        let _ = find_sessions(&path, cwd, sessions, max_depth - 1);
                    }
                }
            }
        }

        Ok(())
    }

    find_sessions(&sessions_dir, &cwd_str, &mut sessions, 4)?;

    // Sort by modification time (newest first)
    sessions.sort_by(|a, b| b.mtime.cmp(&a.mtime));

    // Limit to recent 50 sessions for performance
    sessions.truncate(50);

    if sessions.is_empty() {
        Err("No sessions found in current working directory".to_string())
    } else {
        Ok(sessions)
    }
}

/// Format session information for display with colors
fn format_session_display(sessions: &[SessionInfo]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if sessions.is_empty() {
        return vec![
            Line::from(""),
            "No sessions found in current working directory.".yellow().into(),
            Line::from(""),
            "Create a new session or navigate to a directory with existing sessions.".dim().into(),
        ];
    }

    // Add header
    lines.push(Line::from(""));
    lines.push("Recent Sessions in Current Directory".green().bold().into());
    lines.push(Line::from(""));

    // Display sessions
    for (idx, session) in sessions.iter().enumerate() {
        let session_line = format!(
            "  {}. {} {}",
            idx + 1,
            session.id,
            format!("({})", session.age)
        );
        lines.push(ansi_escape_line(&session_line));
    }

    lines
}

/// Format the help/legend section
fn format_help_section() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("────────────────────────────────────────────────────────────────").dim(),
        Line::from(""),
        "Key Bindings:".bold().into(),
        ansi_escape_line("  ↑↓ / j/k  Navigate sessions       Enter  Resume selected session"),
        ansi_escape_line("  r         Refresh session list   q / Esc  Close this panel"),
        Line::from(""),
        "Additional Actions:".bold().into(),
        ansi_escape_line("  • Use cxresume command for CLI-based selection"),
        ansi_escape_line("  • Sessions are stored in ~/.codex/sessions"),
        Line::from(""),
        Line::from("────────────────────────────────────────────────────────────────").dim(),
    ]
}

/// Create a comprehensive session selection overlay with help
pub fn create_session_picker_overlay() -> Result<Overlay, String> {
    let sessions = get_cwd_sessions()?;

    let mut content = Vec::new();

    // Add title section
    content.push("".into());
    content.push("CXRESUME SESSION PICKER".bold().cyan().into());
    content.push("".into());

    // Add session list
    if !sessions.is_empty() {
        content.extend(format_session_display(&sessions));
    } else {
        content.push("No sessions available in current working directory".yellow().into());
        content.push("".into());
    }

    // Add help section
    content.extend(format_help_section());

    // Add footer
    content.push("".into());
    content.push(ansi_escape_line("💡 Tip: Press Ctrl+X to refresh at any time"));

    let refresh_callback = Box::new(|| {
        match get_cwd_sessions() {
            Ok(sessions) => {
                let mut result = Vec::new();
                result.push("".into());
                result.push("CXRESUME SESSION PICKER".bold().cyan().into());
                result.push("".into());

                if !sessions.is_empty() {
                    result.extend(format_session_display(&sessions));
                } else {
                    result.push("No sessions available".yellow().into());
                    result.push("".into());
                }

                result.extend(format_help_section());
                result.push("".into());
                result.push(ansi_escape_line("💡 Tip: Press Ctrl+X to refresh at any time"));

                Ok(result)
            }
            Err(e) => {
                let mut error_lines = vec![
                    "".into(),
                    "Error loading sessions".red().bold().into(),
                    "".into(),
                    format!("Details: {}", e).dim().into(),
                    "".into(),
                    "Troubleshooting:".bold().into(),
                    "  • Check ~/.codex/sessions directory exists".dim().into(),
                    "  • Verify you have write permissions".dim().into(),
                    "  • Sessions are stored under ~/.codex/sessions/YYYY/MM/DD/".dim().into(),
                    "".into(),
                ];
                error_lines.extend(format_help_section());
                Ok(error_lines)
            }
        }
    });

    Ok(Overlay::new_static_with_title_no_wrap_refresh(
        content,
        "C X R E S U M E   │   ↑/↓:select   j/k:scroll   r:refresh   q/Esc:close   │   C t r l + X"
            .to_string(),
        refresh_callback,
    ))
}
