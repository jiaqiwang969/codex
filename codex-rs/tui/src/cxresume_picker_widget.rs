use crate::pager_overlay::Overlay;
use codex_ansi_escape::ansi_escape_line;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::process::Command;

/// Session metadata extracted from cxresume
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    #[allow(dead_code)]
    pub path: String,
    #[allow(dead_code)]
    pub cwd: String,
    #[allow(dead_code)]
    pub messages_count: u32,
    #[allow(dead_code)]
    pub last_role: String,
    pub age: String,
}

/// Run `cxresume cwd` to get sessions in the current working directory
/// Returns a list of available sessions
pub fn get_cwd_sessions() -> Result<Vec<SessionInfo>, String> {
    // Try to get sessions via cxresume cwd
    let output = Command::new("cxresume")
        .arg("cwd")
        .output()
        .map_err(|e| format!("Failed to execute cxresume: {}", e))?;

    if !output.status.success() {
        // Fallback to plain --list if cwd mode fails
        let fallback = Command::new("cxresume")
            .arg("--list")
            .output()
            .map_err(|e| format!("Failed to list sessions: {}", e))?;

        if !fallback.status.success() {
            return Err("No sessions found".to_string());
        }

        return parse_session_list(&String::from_utf8_lossy(&fallback.stdout));
    }

    parse_session_list(&String::from_utf8_lossy(&output.stdout))
}

fn parse_session_list(output: &str) -> Result<Vec<SessionInfo>, String> {
    let mut sessions = Vec::new();

    // Parse cxresume output format:
    // - session-id (timestamp)
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("Found") || line.starts_with("No session") {
            continue;
        }

        // Try to parse: - <session-id> (timestamp)
        if line.starts_with("- ") {
            let parts: Vec<&str> = line[2..].split(" (").collect();
            if parts.len() >= 1 {
                let id = parts[0].trim().to_string();
                let age = if parts.len() > 1 {
                    parts[1].trim_end_matches(")").to_string()
                } else {
                    "unknown".to_string()
                };

                sessions.push(SessionInfo {
                    id: id.clone(),
                    path: id.clone(),
                    cwd: String::new(),
                    messages_count: 0,
                    last_role: "unknown".to_string(),
                    age,
                });
            }
        }
    }

    if sessions.is_empty() {
        Err("No sessions found in current directory".to_string())
    } else {
        Ok(sessions)
    }
}

/// Resume a session by ID
#[allow(dead_code)]
pub fn resume_session(_session_id: &str) -> Result<(), String> {
    let output = Command::new("cxresume")
        .arg("cwd")
        .arg("-l")
        .output()
        .map_err(|e| format!("Failed to resume session: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Resume failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
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
        ansi_escape_line("  • Use cxresume command directly for advanced options"),
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
                    "  • Ensure cxresume is installed: npm install -g cxresume".dim().into(),
                    "  • Check ~/.codex/sessions directory exists".dim().into(),
                    "  • Verify you have write permissions".dim().into(),
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
