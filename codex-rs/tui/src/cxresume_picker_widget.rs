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

/// Split View Layout Manager for dual-panel picker
#[derive(Debug, Clone)]
pub struct SplitLayout {
    pub left_width: u16,      // Left panel width (35%)
    pub right_width: u16,     // Right panel width (65%)
    pub total_height: u16,
    pub total_width: u16,
    pub gap: u16,             // Space between panels
}

impl SplitLayout {
    /// Create a new split layout from total dimensions
    pub fn new(total_width: u16, total_height: u16) -> Self {
        // Account for 1-char gap between panels
        let usable_width = total_width.saturating_sub(1);

        // 35% left, 65% right
        let left_width = (usable_width as f32 * 0.35) as u16;
        let right_width = usable_width.saturating_sub(left_width);

        SplitLayout {
            left_width,
            right_width,
            total_height,
            total_width,
            gap: 1,
        }
    }

    /// Get the left panel area (0, 0, left_width, total_height)
    pub fn left_area(&self) -> (u16, u16, u16, u16) {
        (0, 0, self.left_width, self.total_height)
    }

    /// Get the right panel area
    pub fn right_area(&self) -> (u16, u16, u16, u16) {
        let x = self.left_width + self.gap;
        (x, 0, self.right_width, self.total_height)
    }
}

/// View mode for the picker
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Split,        // Dual-panel view (default)
    FullPreview,  // Full-screen message preview
    SessionOnly,  // Full-screen session list
}

/// State management for the session picker
#[derive(Debug, Clone)]
pub struct PickerState {
    pub sessions: Vec<SessionInfo>,
    pub selected_idx: usize,
    pub scroll_offset_left: usize,      // For left panel scrolling
    pub scroll_offset_right: usize,     // For right panel scrolling
    pub current_page: usize,            // For pagination
    pub view_mode: ViewMode,
    pub modal_active: bool,             // Delete or edit confirmation dialog
    pub modal_message: String,          // Message to display in modal
}

impl PickerState {
    /// Create a new picker state from sessions list
    pub fn new(sessions: Vec<SessionInfo>) -> Self {
        PickerState {
            sessions,
            selected_idx: 0,
            scroll_offset_left: 0,
            scroll_offset_right: 0,
            current_page: 0,
            view_mode: ViewMode::Split,
            modal_active: false,
            modal_message: String::new(),
        }
    }

    /// Get currently selected session
    pub fn selected_session(&self) -> Option<&SessionInfo> {
        self.sessions.get(self.selected_idx)
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
            self.scroll_offset_right = 0; // Reset preview scroll
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_idx < self.sessions.len().saturating_sub(1) {
            self.selected_idx += 1;
            self.scroll_offset_right = 0; // Reset preview scroll
        }
    }

    /// Jump to first session
    pub fn select_first(&mut self) {
        self.selected_idx = 0;
        self.scroll_offset_right = 0;
    }

    /// Jump to last session
    pub fn select_last(&mut self) {
        self.selected_idx = self.sessions.len().saturating_sub(1);
        self.scroll_offset_right = 0;
    }

    /// Page up (jump 5 items)
    pub fn page_prev(&mut self) {
        self.selected_idx = self.selected_idx.saturating_sub(5);
        self.scroll_offset_right = 0;
    }

    /// Page down (jump 5 items)
    pub fn page_next(&mut self) {
        self.selected_idx = (self.selected_idx + 5).min(self.sessions.len().saturating_sub(1));
        self.scroll_offset_right = 0;
    }

    /// Scroll preview up
    pub fn scroll_preview_up(&mut self) {
        self.scroll_offset_right = self.scroll_offset_right.saturating_sub(1);
    }

    /// Scroll preview down
    pub fn scroll_preview_down(&mut self) {
        self.scroll_offset_right = self.scroll_offset_right.saturating_add(1);
    }

    /// Toggle view mode
    pub fn toggle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Split => ViewMode::FullPreview,
            ViewMode::FullPreview => ViewMode::SessionOnly,
            ViewMode::SessionOnly => ViewMode::Split,
        };
    }

    /// Open delete confirmation modal
    pub fn confirm_delete(&mut self) {
        if let Some(session) = self.sessions.get(self.selected_idx) {
            self.modal_active = true;
            self.modal_message = format!(
                "Delete session '{}'?\nThis action cannot be undone.\n\nPress 'y' to confirm or 'n' to cancel.",
                session.id
            );
        }
    }

    /// Close any active modal
    pub fn close_modal(&mut self) {
        self.modal_active = false;
        self.modal_message.clear();
    }
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

/// Format left panel: session list with 3 lines per session
fn format_left_panel_sessions(sessions: &[SessionInfo], selected_idx: Option<usize>, _width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if sessions.is_empty() {
        lines.push("No sessions".yellow().into());
        return lines;
    }

    // Header with pagination info
    let header = format!(
        "SESSIONS  │  Page 1/1 │ Showing {}/{}",
        sessions.len(),
        sessions.len()
    );
    lines.push(ansi_escape_line(&header).bold());
    lines.push(Line::from(""));

    // Display sessions (3 lines per session + 1 blank line spacing)
    for (idx, session) in sessions.iter().enumerate() {
        let is_selected = selected_idx == Some(idx);
        let marker = if is_selected { "▶" } else { " " };

        // Line 1: ID, age
        let line1_text = format!(
            " {} {}  ({})",
            marker,
            session.id.as_str().cyan(),
            session.age.as_str().dim()
        );
        let mut line1 = ansi_escape_line(&line1_text);
        if is_selected {
            line1 = line1.reversed();
        }
        lines.push(line1);

        // Line 2: CWD or path
        let line2_text = format!("   {}", session.cwd.as_str().dim());
        let mut line2 = ansi_escape_line(&line2_text);
        if is_selected {
            line2 = line2.reversed();
        }
        lines.push(line2);

        // Line 3: Messages + Model + Last role
        let line3_text = format!(
            "   {} messages • {} • {}",
            session.message_count.to_string().yellow(),
            session.model.as_str().cyan(),
            format!("Last: {}", session.last_role).green()
        );
        let mut line3 = ansi_escape_line(&line3_text);
        if is_selected {
            line3 = line3.reversed();
        }
        lines.push(line3);

        // Spacing
        lines.push(Line::from(""));
    }

    lines
}

/// Format right panel: message preview with block-style format
fn format_right_panel_preview(session: &SessionInfo, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Info box
    let info = format!(
        "▸ Session: {} │ Path: {}",
        session.id.as_str().cyan(),
        session.cwd.as_str().dim()
    );
    lines.push(ansi_escape_line(&info));
    lines.push(Line::from("─".repeat(width as usize)));
    lines.push(Line::from(""));

    // Message blocks
    let messages = extract_recent_messages_with_timestamps(&session.path, 6);
    if messages.is_empty() {
        lines.push("No messages".yellow().into());
    } else {
        for (role, content, _timestamp) in messages.iter() {
            // Block header with vertical bar
            let role_color = if role == "User" {
                role.as_str().red()
            } else {
                role.as_str().green()
            };

            let header = format!("┃ {}", role_color);
            lines.push(ansi_escape_line(&header));

            // Message content with wrapping
            let max_content_width = width.saturating_sub(2) as usize;
            let mut line_count = 0;
            let line_limit = 3; // Show max 3 lines per message in preview

            for content_line in content.lines() {
                if line_count >= line_limit {
                    lines.push("  ⋮".dim().into());
                    break;
                }

                if content_line.is_empty() {
                    continue;
                }

                if content_line.len() > max_content_width {
                    let chunk = &content_line[..max_content_width.saturating_sub(1)];
                    lines.push(ansi_escape_line(&format!("  {}…", chunk)));
                } else {
                    lines.push(ansi_escape_line(&format!("  {}", content_line)));
                }
                line_count += 1;
            }

            lines.push(Line::from(""));
        }
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

/// Extract messages with timestamps (role, content, timestamp)
fn extract_recent_messages_with_timestamps(path: &PathBuf, limit: usize) -> Vec<(String, String, String)> {
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
                                // Extract timestamp if available, otherwise use empty string
                                let timestamp = payload
                                    .get("timestamp")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("--:--:--")
                                    .to_string();
                                messages.push((role, content.to_string(), timestamp));
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

/// Format message blocks with vertical bar indicator (┃)
/// This creates the block-style preview format used in cxresume JS
fn format_message_blocks(session: &SessionInfo, width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Info box header
    let info_text = format!(
        "Session: {} • Path: {} • Started: {}",
        session.id.as_str().cyan(),
        session.cwd.as_str().dim(),
        session.age.as_str().dim()
    );
    lines.push(ansi_escape_line(&info_text));
    lines.push(Line::from(""));

    let messages = extract_recent_messages_with_timestamps(&session.path, 8);
    if messages.is_empty() {
        lines.push("No messages found in this session.".yellow().into());
    } else {
        for (role, content, _timestamp) in messages.iter() {
            // Message header with bar indicator (┃)
            let role_color = if role == "User" {
                role.as_str().red()
            } else {
                role.as_str().green()
            };

            let header_text = format!("┃ {}", role_color);
            lines.push(ansi_escape_line(&header_text));

            // Message body with wrapping and bar prefix
            let usable_width = width.saturating_sub(3) as usize; // Account for "┃ " prefix
            for content_line in content.lines() {
                // Wrap long lines
                if content_line.len() > usable_width {
                    let mut remaining = content_line;
                    while !remaining.is_empty() {
                        let chunk_size = usable_width.min(remaining.len());
                        let chunk = &remaining[..chunk_size];
                        lines.push(ansi_escape_line(&format!("  {}", chunk)));
                        remaining = &remaining[chunk_size..];
                    }
                } else {
                    lines.push(ansi_escape_line(&format!("  {}", content_line)));
                }
            }

            // Spacing between messages
            lines.push(Line::from(""));
        }
    }

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

/// Create a comprehensive Split View session picker overlay
pub fn create_session_picker_overlay() -> Result<Overlay, String> {
    let sessions = get_cwd_sessions()?;
    let state = PickerState::new(sessions);

    // Render with initial state
    let content = render_picker_view(&state)?;

    let refresh_callback = Box::new(|| {
        match get_cwd_sessions() {
            Ok(sessions) => {
                let state = PickerState::new(sessions);
                render_picker_view(&state)
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
                error_lines.push(Line::from("────────────────────────────────────────────────────────────────").dim());
                error_lines.push("Press q to close".dim().into());
                Ok(error_lines)
            }
        }
    });

    Ok(Overlay::new_static_with_title_no_wrap_refresh(
        content,
        "S E S S I O N   P I C K E R   │   ↑/↓:select   j/k:scroll   Enter:resume   d:delete   q/Esc:close   │   Ctrl+X"
            .to_string(),
        refresh_callback,
    ))
}

/// Render the picker view based on current state
fn render_picker_view(state: &PickerState) -> Result<Vec<Line<'static>>, String> {
    let layout = SplitLayout::new(120, 30);
    let mut content = Vec::new();

    // Add title bar
    content.push("".into());
    let title = format!(
        "    C X R E S U M E   S E S S I O N   P I C K E R    ({} sessions) │ Mode: {:?}",
        state.sessions.len(),
        state.view_mode
    );
    content.push(ansi_escape_line(&title).bold().cyan());
    content.push("".into());

    if state.sessions.is_empty() {
        content.push("No sessions found in current working directory".yellow().into());
        content.push("".into());
        content.push("Press q to close".dim().into());
    } else {
        match state.view_mode {
            ViewMode::Split => {
                // Render split view
                content.extend(format_left_panel_sessions(&state.sessions, Some(state.selected_idx), layout.left_width));
                content.push(Line::from(""));
                content.push("─────  ▼ RIGHT PANEL PREVIEW ▼  ─────".dim().into());
                content.push(Line::from(""));

                if let Some(selected_session) = state.selected_session() {
                    content.extend(format_right_panel_preview(selected_session, layout.right_width));
                }
            }
            ViewMode::FullPreview => {
                // Full screen preview of selected session
                content.push("".into());
                if let Some(selected_session) = state.selected_session() {
                    content.extend(format_session_preview(selected_session));
                }
            }
            ViewMode::SessionOnly => {
                // Full screen session list
                content.extend(format_left_panel_sessions(&state.sessions, Some(state.selected_idx), 120));
            }
        }
    }

    // Modal overlay if active
    if state.modal_active {
        content.push(Line::from(""));
        content.push(Line::from("╔════════════════════════════════════════════════════════════╗").dim());
        for line in state.modal_message.lines() {
            content.push(ansi_escape_line(&format!("║ {} ", line)).dim());
        }
        content.push(Line::from("╚════════════════════════════════════════════════════════════╝").dim());
    }

    // Add footer with instructions
    content.push(Line::from(""));
    content.push(Line::from("────────────────────────────────────────────────────────────────").dim());
    content.push("Keyboard Shortcuts:".bold().into());
    content.push(ansi_escape_line("  ↑↓      Navigate sessions       j/k    Scroll preview      Page↑/↓  Page jump"));
    content.push(ansi_escape_line("  Enter   Resume session         d      Delete              f        Full preview"));
    content.push(ansi_escape_line("  n       New session            c      Copy ID             q/Esc    Close"));
    content.push(Line::from(""));

    Ok(content)
}
