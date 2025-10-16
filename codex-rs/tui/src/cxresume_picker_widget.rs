use crate::pager_overlay::Overlay;
use codex_ansi_escape::ansi_escape_line;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::fs;
use std::io::BufRead;
use std::path::PathBuf;

/// Enhanced session metadata with comprehensive information
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub path: PathBuf,
    pub cwd: String,
    pub age: String,
    pub mtime: u64,
    pub message_count: usize,
    pub last_role: String,
    pub total_tokens: usize,
    pub model: String,
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

/// Extract enhanced session metadata from .jsonl file
fn extract_session_meta(path: &PathBuf) -> Result<(String, String, usize, String, usize, String), String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);
    let mut lines = reader.lines();

    let mut session_id = String::new();
    let mut cwd = String::new();
    let mut model = String::from("unknown");
    let mut message_count = 0;
    let mut last_role = String::from("-");
    let mut total_tokens = 0;

    // First pass: extract session metadata from first line
    if let Some(Ok(first_line)) = lines.next() {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&first_line) {
            if let Some(payload) = json.get("payload") {
                session_id = payload
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| path.file_name().unwrap().to_string_lossy().to_string());

                cwd = payload
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();

                model = payload
                    .get("model")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
            }
        }
    }

    // Second pass: count messages and extract last role and token usage
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let reader = std::io::BufReader::new(file);
    for line in reader.lines() {
        if let Ok(line) = line {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                let msg_type = json
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                match msg_type {
                    "user_message" | "assistant_message" => {
                        message_count += 1;
                        last_role = if msg_type == "user_message" {
                            "User".to_string()
                        } else {
                            "Assistant".to_string()
                        };

                        // Try to extract token count from usage if available
                        if let Some(usage) = json.get("payload").and_then(|p| p.get("usage")) {
                            if let Some(total) = usage.get("total_tokens").and_then(|t| t.as_u64()) {
                                total_tokens = total as usize;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if session_id.is_empty() {
        session_id = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
    }

    Ok((session_id, cwd, message_count, last_role, total_tokens, model))
}

/// Extract recent messages from a session file for preview
fn extract_recent_messages(path: &PathBuf, limit: usize) -> Vec<(String, String)> {
    let mut messages = Vec::new();

    if let Ok(file) = fs::File::open(path) {
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    let msg_type = json
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if msg_type == "user_message" || msg_type == "assistant_message" {
                        if let Some(payload) = json.get("payload") {
                            if let Some(content) = payload.get("content").and_then(|c| c.as_str()) {
                                let role = if msg_type == "user_message" { "User" } else { "Assistant" }.to_string();
                                messages.push((role, content.to_string()));
                            }
                        }
                    }
                }
            }
        }
    }

    // Keep only the last 'limit' messages
    if messages.len() > limit {
        messages.drain(0..messages.len() - limit);
    }

    messages
}

/// Format relative time in human-readable format
fn format_relative_time(mtime: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let diff = now.saturating_sub(mtime);

    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 604800 {
        format!("{}d ago", diff / 86400)
    } else if diff < 2592000 {
        format!("{}w ago", diff / 604800)
    } else if diff < 31536000 {
        format!("{}mo ago", diff / 2592000)
    } else {
        format!("{}y ago", diff / 31536000)
    }
}

/// Get sessions in current working directory with enhanced metadata
pub fn get_cwd_sessions() -> Result<Vec<SessionInfo>, String> {
    let cwd = get_cwd()?;
    let sessions_dir = get_sessions_dir()?;

    let cwd_str = cwd.to_string_lossy().to_string();
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
                        if let Ok((id, session_cwd, msg_count, last_role, tokens, model)) =
                            extract_session_meta(&path)
                        {
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
                                    message_count: msg_count,
                                    last_role,
                                    total_tokens: tokens,
                                    model,
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

    // Limit to recent 100 sessions for performance
    sessions.truncate(100);

    if sessions.is_empty() {
        Err("No sessions found in current working directory".to_string())
    } else {
        Ok(sessions)
    }
}

/// Format session information for display with colors and detailed info
fn format_session_display(sessions: &[SessionInfo], selected_idx: Option<usize>) -> Vec<Line<'static>> {
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

    // Display sessions with enhanced information (two lines per session)
    for (idx, session) in sessions.iter().enumerate() {
        let is_selected = selected_idx == Some(idx);
        let marker = if is_selected { "▶ " } else { "  " };

        // First line: ID, age, message count, last role
        let session_line = format!(
            "{}{}.  {} {} │ {} messages │ {}",
            marker,
            idx + 1,
            session.id.as_str().cyan(),
            format!("({})", session.age).dim(),
            session.message_count.to_string().yellow(),
            format!("Last: {}", session.last_role).green()
        );
        let mut line1 = ansi_escape_line(&session_line);
        if is_selected {
            line1 = line1.reversed();
        }
        lines.push(line1);

        // Second line: Model and token usage
        let model_line = format!(
            "      Model: {} │ Tokens: {}",
            session.model.as_str().cyan(),
            session.total_tokens.to_string().yellow()
        );
        let mut line2 = ansi_escape_line(&model_line);
        if is_selected {
            line2 = line2.reversed();
        }
        lines.push(line2);

        // Add spacing between sessions
        lines.push(Line::from(""));
    }

    lines
}

/// Format the help/legend section with key bindings and information
fn format_help_section() -> Vec<Line<'static>> {
    vec![
        Line::from(""),
        Line::from("────────────────────────────────────────────────────────────────").dim(),
        Line::from(""),
        "Key Bindings:".bold().into(),
        ansi_escape_line("  ↑↓ / j/k  Navigate sessions       Enter  Resume selected session"),
        ansi_escape_line("  i         Session info           p      Preview messages"),
        ansi_escape_line("  d         Delete session         r      Refresh session list"),
        ansi_escape_line("  q / Esc   Close this panel       /      Search sessions"),
        Line::from(""),
        "Display Information:".bold().into(),
        ansi_escape_line("  • Messages: Total user + assistant messages in this session"),
        ansi_escape_line("  • Last: Last message type in session (User or Assistant)"),
        ansi_escape_line("  • Model: AI model used for this session"),
        ansi_escape_line("  • Tokens: Total tokens consumed in this session"),
        Line::from(""),
        Line::from("────────────────────────────────────────────────────────────────").dim(),
    ]
}

/// Format detailed session information for info modal
fn format_session_details(session: &SessionInfo) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(""));
    lines.push("SESSION DETAILS".bold().cyan().into());
    lines.push(Line::from(""));

    // Session ID
    lines.push(ansi_escape_line(&format!(
        "  ID:             {}",
        session.id.as_str().cyan()
    )));

    // Model
    lines.push(ansi_escape_line(&format!(
        "  Model:          {}",
        session.model.as_str().yellow()
    )));

    // Messages
    lines.push(ansi_escape_line(&format!(
        "  Messages:       {} ({} last)",
        session.message_count.to_string().yellow(),
        session.last_role.as_str().green()
    )));

    // Tokens
    lines.push(ansi_escape_line(&format!(
        "  Tokens Used:    {}",
        session.total_tokens.to_string().yellow()
    )));

    // Age
    lines.push(ansi_escape_line(&format!(
        "  Last Activity:  {}",
        session.age.as_str().dim()
    )));

    // Working Directory
    if !session.cwd.is_empty() {
        lines.push(ansi_escape_line(&format!(
            "  Directory:      {}",
            session.cwd.as_str().dim()
        )));
    }

    // File Path
    lines.push(ansi_escape_line(&format!(
        "  File Path:      {}",
        session.path.display().to_string().as_str().dim()
    )));

    lines.push(Line::from(""));
    lines.push(Line::from("────────────────────────────────────────────────────────────────").dim());
    lines.push(Line::from(""));
    lines.push("STATISTICS".bold().cyan().into());
    lines.push(Line::from(""));

    lines.push(ansi_escape_line(&format!(
        "  Average tokens per message: {}",
        if session.message_count > 0 {
            (session.total_tokens / session.message_count).to_string()
        } else {
            "N/A".to_string()
        }
        .as_str()
        .yellow()
    )));

    lines.push(Line::from(""));
    lines.push("Actions:".bold().into());
    lines.push(ansi_escape_line("  Enter      Resume this session"));
    lines.push(ansi_escape_line("  p          Preview recent messages"));
    lines.push(ansi_escape_line("  d          Delete this session"));
    lines.push(ansi_escape_line("  q / Esc    Back to session list"));
    lines.push(Line::from(""));

    lines
}

/// Format session preview with recent messages
fn format_session_preview(session: &SessionInfo) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    lines.push(Line::from(""));
    lines.push("SESSION PREVIEW - Recent Messages".bold().cyan().into());
    lines.push(Line::from(""));
    lines.push(ansi_escape_line(&format!("Session: {}", session.id.as_str().cyan())));
    lines.push(ansi_escape_line(&format!("Model: {}", session.model.as_str().yellow())));
    lines.push(Line::from(""));
    lines.push(Line::from("────────────────────────────────────────────────────────────────").dim());
    lines.push(Line::from(""));

    let messages = extract_recent_messages(&session.path, 5);
    if messages.is_empty() {
        lines.push("No messages found in this session.".yellow().into());
    } else {
        for (idx, (role, content)) in messages.iter().enumerate() {
            // Role header
            let role_line = if role == "User" {
                format!("{}. {} (User)", idx + 1, "▶".cyan())
            } else {
                format!("{}. {} (Assistant)", idx + 1, "◀".green())
            };
            lines.push(role_line.into());

            // Message content - truncate long messages and wrap
            let truncated = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };

            // Split into lines and add with indentation
            for msg_line in truncated.lines() {
                lines.push(Line::from(format!("   {}", msg_line).dim()));
            }
            lines.push(Line::from(""));
        }
    }

    lines.push(Line::from("────────────────────────────────────────────────────────────────").dim());
    lines.push(Line::from(""));
    lines.push("  q / Esc  Back to session list".dim().into());
    lines.push(Line::from(""));

    lines
}

/// Create a comprehensive session selection overlay with enhanced features
pub fn create_session_picker_overlay() -> Result<Overlay, String> {
    let sessions = get_cwd_sessions()?;

    let mut content = Vec::new();

    // Add title section
    content.push("".into());
    content.push("CXRESUME SESSION PICKER - ENHANCED".bold().cyan().into());
    content.push("".into());

    // Add session list with enhanced metadata (show first session as selected)
    if !sessions.is_empty() {
        content.extend(format_session_display(&sessions, Some(0)));
    } else {
        content.push("No sessions available in current working directory".yellow().into());
        content.push("".into());
    }

    // Add help section
    content.extend(format_help_section());

    // Add footer with statistics
    content.push("".into());
    let total_sessions = sessions.len();
    let total_messages: usize = sessions.iter().map(|s| s.message_count).sum();
    let avg_messages = if total_sessions > 0 {
        total_messages / total_sessions
    } else {
        0
    };
    let footer = format!(
        "💡 Statistics: {} sessions | {} total messages | {} avg/session",
        total_sessions, total_messages, avg_messages
    );
    content.push(ansi_escape_line(&footer));

    let refresh_callback = Box::new(|| {
        match get_cwd_sessions() {
            Ok(sessions) => {
                let mut result = Vec::new();
                result.push("".into());
                result.push("CXRESUME SESSION PICKER - ENHANCED".bold().cyan().into());
                result.push("".into());

                if !sessions.is_empty() {
                    result.extend(format_session_display(&sessions, Some(0)));
                } else {
                    result.push("No sessions available".yellow().into());
                    result.push("".into());
                }

                result.extend(format_help_section());
                result.push("".into());
                let total_sessions = sessions.len();
                let total_messages: usize = sessions.iter().map(|s| s.message_count).sum();
                let avg_messages = if total_sessions > 0 {
                    total_messages / total_sessions
                } else {
                    0
                };
                let footer = format!(
                    "💡 Statistics: {} sessions | {} total messages | {} avg/session",
                    total_sessions, total_messages, avg_messages
                );
                result.push(ansi_escape_line(&footer));

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
                    "  • Verify you have read permissions".dim().into(),
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
        "C X R E S U M E   │   ↑/↓:select   j/k:scroll   i:info   r:refresh   d:delete   q/Esc:close   │   C t r l + X"
            .to_string(),
        refresh_callback,
    ))
}
