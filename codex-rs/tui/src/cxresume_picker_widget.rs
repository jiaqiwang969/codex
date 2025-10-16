use crate::pager_overlay::Overlay;
use codex_ansi_escape::ansi_escape_line;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::collections::HashMap;
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
    #[allow(dead_code)]
    pub total_tokens: usize,
    pub model: String,
}

/// Cached preview data for a session (messages and metadata)
#[derive(Debug, Clone)]
struct PreviewCache {
    #[allow(dead_code)]
    messages: Vec<(String, String, String)>, // (role, content, timestamp)
    #[allow(dead_code)]
    cached_at: u64,                           // Unix timestamp when cached
}

/// Message summary for quick access (count and last role)
#[derive(Debug, Clone)]
pub struct MessageSummary {
    #[allow(dead_code)]
    message_count: usize,
    #[allow(dead_code)]
    last_role: String,
    #[allow(dead_code)]
    last_update: u64,
}

/// Multi-layered cache for session picker performance
/// Stores metadata, previews, and message summaries to avoid repeated file I/O
#[derive(Debug, Clone)]
pub struct CacheLayer {
    // Session metadata cache (keyed by file path)
    #[allow(dead_code)]
    meta_cache: HashMap<PathBuf, SessionInfo>,

    // Preview cache (keyed by session ID) - stores formatted message previews
    preview_cache: HashMap<String, PreviewCache>,

    // Message summary cache (keyed by file path) - lightweight alternative to full preview
    #[allow(dead_code)]
    summary_cache: HashMap<PathBuf, MessageSummary>,

    // Cache hit/miss statistics
    #[allow(dead_code)]
    meta_hits: usize,
    #[allow(dead_code)]
    meta_misses: usize,
    #[allow(dead_code)]
    preview_hits: usize,
    #[allow(dead_code)]
    preview_misses: usize,
}

impl CacheLayer {
    /// Create a new empty cache layer
    pub fn new() -> Self {
        CacheLayer {
            meta_cache: HashMap::new(),
            preview_cache: HashMap::new(),
            summary_cache: HashMap::new(),
            meta_hits: 0,
            meta_misses: 0,
            preview_hits: 0,
            preview_misses: 0,
        }
    }

    /// Get or insert session metadata in cache
    #[allow(dead_code)]
    pub fn get_or_insert_meta(
        &mut self,
        path: &PathBuf,
        default: SessionInfo,
    ) -> SessionInfo {
        if self.meta_cache.contains_key(path) {
            self.meta_hits += 1;
            self.meta_cache[path].clone()
        } else {
            self.meta_misses += 1;
            self.meta_cache.insert(path.clone(), default.clone());
            default
        }
    }

    /// Get preview from cache if available
    pub fn get_preview(&mut self, session_id: &str) -> Option<Vec<(String, String, String)>> {
        if let Some(cached) = self.preview_cache.get(session_id) {
            self.preview_hits += 1;
            Some(cached.messages.clone())
        } else {
            self.preview_misses += 1;
            None
        }
    }

    /// Store preview in cache
    pub fn cache_preview(
        &mut self,
        session_id: String,
        messages: Vec<(String, String, String)>,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.preview_cache.insert(
            session_id,
            PreviewCache {
                messages,
                cached_at: now,
            },
        );
    }

    /// Get message summary from cache if available
    #[allow(dead_code)]
    pub fn get_summary(&mut self, path: &PathBuf) -> Option<MessageSummary> {
        self.summary_cache.get(path).cloned()
    }

    /// Store message summary in cache
    #[allow(dead_code)]
    pub fn cache_summary(
        &mut self,
        path: PathBuf,
        message_count: usize,
        last_role: String,
    ) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.summary_cache.insert(
            path,
            MessageSummary {
                message_count,
                last_role,
                last_update: now,
            },
        );
    }

    /// Remove a specific preview from cache
    pub fn remove_preview(&mut self, session_id: &str) {
        self.preview_cache.remove(session_id);
    }

    /// Clear all caches (useful for refresh operations)
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.meta_cache.clear();
        self.preview_cache.clear();
        self.summary_cache.clear();
    }

    /// Get cache statistics for debugging
    #[allow(dead_code)]
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        (
            self.meta_hits,
            self.meta_misses,
            self.preview_hits,
            self.preview_misses,
        )
    }
}

impl Default for CacheLayer {
    fn default() -> Self {
        Self::new()
    }
}


/// Split View Layout Manager for dual-panel picker
#[derive(Debug, Clone)]
pub struct SplitLayout {
    pub left_width: u16,      // Left panel width (35%)
    pub right_width: u16,     // Right panel width (65%)
    #[allow(dead_code)]
    pub total_height: u16,
    #[allow(dead_code)]
    pub total_width: u16,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
    pub fn left_area(&self) -> (u16, u16, u16, u16) {
        (0, 0, self.left_width, self.total_height)
    }

    /// Get the right panel area
    #[allow(dead_code)]
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

/// Pagination manager for session lists
#[derive(Debug, Clone)]
pub struct Pagination {
    pub total_items: usize,
    pub items_per_page: usize,
    pub current_page: usize,
}

impl Pagination {
    /// Create new pagination from total items
    pub fn new(total_items: usize, items_per_page: usize) -> Self {
        Pagination {
            total_items,
            items_per_page,
            current_page: 0,
        }
    }

    /// Get the total number of pages
    pub fn total_pages(&self) -> usize {
        (self.total_items + self.items_per_page - 1) / self.items_per_page
    }

    /// Get the start index for current page
    pub fn page_start(&self) -> usize {
        self.current_page * self.items_per_page
    }

    /// Get the end index for current page (exclusive)
    pub fn page_end(&self) -> usize {
        ((self.current_page + 1) * self.items_per_page).min(self.total_items)
    }

    /// Get items for the current page (slice indices)
    pub fn page_range(&self) -> std::ops::Range<usize> {
        self.page_start()..self.page_end()
    }

    /// Move to next page
    pub fn next_page(&mut self) -> bool {
        if self.current_page + 1 < self.total_pages() {
            self.current_page += 1;
            true
        } else {
            false
        }
    }

    /// Move to previous page
    pub fn prev_page(&mut self) -> bool {
        if self.current_page > 0 {
            self.current_page -= 1;
            true
        } else {
            false
        }
    }

    /// Jump to first page
    #[allow(dead_code)]
    pub fn first_page(&mut self) {
        self.current_page = 0;
    }

    /// Jump to last page
    #[allow(dead_code)]
    pub fn last_page(&mut self) {
        self.current_page = self.total_pages().saturating_sub(1);
    }

    /// Check if there's a next page
    #[allow(dead_code)]
    pub fn has_next(&self) -> bool {
        self.current_page + 1 < self.total_pages()
    }

    /// Check if there's a previous page
    #[allow(dead_code)]
    pub fn has_prev(&self) -> bool {
        self.current_page > 0
    }
}

/// State management for the session picker
#[derive(Debug, Clone)]
pub struct PickerState {
    pub sessions: Vec<SessionInfo>,
    pub selected_idx: usize,
    #[allow(dead_code)]
    pub scroll_offset_left: usize,      // For left panel scrolling
    pub scroll_offset_right: usize,     // For right panel scrolling
    pub pagination: Pagination,         // Pagination manager
    pub view_mode: ViewMode,
    pub modal_active: bool,             // Delete or edit confirmation dialog
    pub modal_message: String,          // Message to display in modal
    pub cache: CacheLayer,              // Multi-layered cache for performance
}

impl PickerState {
    /// Create a new picker state from sessions list
    pub fn new(sessions: Vec<SessionInfo>) -> Self {
        let items_count = sessions.len();
        let pagination = Pagination::new(items_count, 30); // 30 items per page
        let mut picker_state = PickerState {
            sessions,
            selected_idx: 0,
            scroll_offset_left: 0,
            scroll_offset_right: 0,
            pagination,
            view_mode: ViewMode::Split,
            modal_active: false,
            modal_message: String::new(),
            cache: CacheLayer::new(),
        };
        // Prefetch the first visible page of sessions on initial load
        picker_state.prefetch_visible_page();
        picker_state
    }

    /// Get currently selected session
    pub fn selected_session(&self) -> Option<&SessionInfo> {
        self.sessions.get(self.selected_idx)
    }

    /// Get sessions for the current page
    #[allow(dead_code)]
    pub fn current_page_sessions(&self) -> &[SessionInfo] {
        let range = self.pagination.page_range();
        &self.sessions[range]
    }

    /// Move to next page and reset selection to first item on page
    pub fn next_page(&mut self) {
        if self.pagination.next_page() {
            self.selected_idx = self.pagination.page_start();
            self.scroll_offset_right = 0;
            // Prefetch visible page for faster display
            self.prefetch_visible_page();
        }
    }

    /// Move to previous page and reset selection to first item on page
    pub fn prev_page(&mut self) {
        if self.pagination.prev_page() {
            self.selected_idx = self.pagination.page_start();
            self.scroll_offset_right = 0;
            // Prefetch visible page for faster display
            self.prefetch_visible_page();
        }
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        if self.selected_idx > 0 {
            self.selected_idx -= 1;
            self.scroll_offset_right = 0; // Reset preview scroll
            // Prefetch adjacent sessions for smooth navigation
            self.prefetch_adjacent_sessions();
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_idx < self.sessions.len().saturating_sub(1) {
            self.selected_idx += 1;
            self.scroll_offset_right = 0; // Reset preview scroll
            // Prefetch adjacent sessions for smooth navigation
            self.prefetch_adjacent_sessions();
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

    /// Get cached preview or fetch it from file
    #[allow(dead_code)]
    pub fn get_or_fetch_preview(&mut self, session: &SessionInfo, limit: usize) -> Vec<(String, String, String)> {
        // Try to get from cache first
        if let Some(cached) = self.cache.get_preview(&session.id) {
            return cached;
        }

        // Not in cache, extract from file and cache it
        let messages = extract_recent_messages_with_timestamps(&session.path, limit);
        self.cache.cache_preview(session.id.clone(), messages.clone());
        messages
    }

    /// Get cache statistics
    #[allow(dead_code)]
    pub fn cache_stats(&self) -> (usize, usize, usize, usize) {
        self.cache.stats()
    }

    /// Clear the entire cache (for refresh operations)
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Prefetch preview for session at index (non-blocking optimization)
    /// This method loads the preview into cache if not already cached
    pub fn prefetch_preview_for_index(&mut self, idx: usize) {
        if let Some(session) = self.sessions.get(idx) {
            // Only prefetch if not already in cache
            if self.cache.get_preview(&session.id).is_none() {
                let messages = extract_recent_messages_with_timestamps(&session.path, 6);
                self.cache.cache_preview(session.id.clone(), messages);
            }
        }
    }

    /// Prefetch adjacent sessions (previous and next) when navigating
    /// This provides lazy loading benefit without blocking the UI
    pub fn prefetch_adjacent_sessions(&mut self) {
        // Prefetch next session
        if self.selected_idx + 1 < self.sessions.len() {
            self.prefetch_preview_for_index(self.selected_idx + 1);
        }

        // Prefetch previous session
        if self.selected_idx > 0 {
            self.prefetch_preview_for_index(self.selected_idx - 1);
        }
    }

    /// Prefetch visible page of sessions for immediate display
    /// Useful when paginating or view first loads
    pub fn prefetch_visible_page(&mut self) {
        let range = self.pagination.page_range();
        for idx in range {
            self.prefetch_preview_for_index(idx);
        }
    }
}

/// Event type enum for picker keyboard input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerEvent {
    // Navigation
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
    PageNext,
    PagePrev,

    // Preview scrolling
    ScrollUp,
    ScrollDown,

    // Actions
    Resume,           // Enter key - return selected session
    Delete,           // d key - confirm delete
    #[allow(dead_code)]
    ToggleViewMode,   // f key - cycle through views
    CopySessionId,    // c key - copy to clipboard
    NewSession,       // n key - create new

    // Navigation modes
    CycleViewMode,    // f key - Split → FullPreview → SessionOnly → Split
    Refresh,          // r key - refresh sessions list

    // Dialog control
    ConfirmAction,    // y key in modal
    #[allow(dead_code)]
    CancelAction,     // n key in modal

    // Exit
    Exit,             // q or Esc
}

impl PickerState {
    /// Handle a picker event and update state accordingly
    /// Returns Some(session_id) when: (1) session to resume, (2) session to delete (empty string = exit)
    pub fn handle_event(&mut self, event: PickerEvent) -> Option<String> {
        if self.modal_active {
            // In modal mode, only handle confirm/cancel
            match event {
                PickerEvent::ConfirmAction => {
                    // Confirm delete: remove session file and from list
                    if let Some(session) = self.sessions.get(self.selected_idx) {
                        let session_id = session.id.clone();
                        let session_path = session.path.clone();
                        self.modal_active = false;

                        // Remove from sessions list
                        self.sessions.remove(self.selected_idx);

                        // Adjust selected index if needed
                        if self.selected_idx >= self.sessions.len() && self.selected_idx > 0 {
                            self.selected_idx -= 1;
                        }

                        // Update pagination total
                        self.pagination.total_items = self.sessions.len();

                        // Delete the file
                        let _ = fs::remove_file(&session_path);

                        // Clear cache entries for this session
                        self.cache.remove_preview(&session_id);
                    }
                }
                PickerEvent::CancelAction => {
                    self.close_modal();
                }
                _ => {}
            }
            return None;
        }

        // Normal mode event handling
        match event {
            PickerEvent::SelectNext => self.select_next(),
            PickerEvent::SelectPrev => self.select_prev(),
            PickerEvent::SelectFirst => self.select_first(),
            PickerEvent::SelectLast => self.select_last(),
            PickerEvent::PageNext => self.next_page(),
            PickerEvent::PagePrev => self.prev_page(),

            PickerEvent::ScrollUp => self.scroll_preview_up(),
            PickerEvent::ScrollDown => self.scroll_preview_down(),

            PickerEvent::ToggleViewMode | PickerEvent::CycleViewMode => {
                self.toggle_view_mode();
            }

            PickerEvent::Resume => {
                if let Some(session) = self.selected_session() {
                    return Some(session.id.clone());
                }
            }

            PickerEvent::Delete => {
                self.confirm_delete();
            }

            PickerEvent::CopySessionId => {
                if let Some(_session) = self.selected_session() {
                    // Would copy to clipboard: _session.id.clone()
                }
            }

            PickerEvent::NewSession => {
                // Would create new session
            }

            PickerEvent::Refresh => {
                // Would trigger refresh callback
            }

            PickerEvent::Exit => {
                return Some(String::new()); // Signal exit
            }

            _ => {}
        }

        None
    }

    /// Convert KeyEvent to PickerEvent (for integration with Overlay)
    pub fn key_to_event(key_code: crossterm::event::KeyCode) -> Option<PickerEvent> {
        use crossterm::event::KeyCode;

        match key_code {
            KeyCode::Up => Some(PickerEvent::SelectPrev),
            KeyCode::Down => Some(PickerEvent::SelectNext),
            KeyCode::Home => Some(PickerEvent::SelectFirst),
            KeyCode::End => Some(PickerEvent::SelectLast),
            KeyCode::PageUp => Some(PickerEvent::PagePrev),
            KeyCode::PageDown => Some(PickerEvent::PageNext),

            KeyCode::Char('j') => Some(PickerEvent::ScrollDown),
            KeyCode::Char('k') => Some(PickerEvent::ScrollUp),

            KeyCode::Enter => Some(PickerEvent::Resume),
            KeyCode::Char('d') => Some(PickerEvent::Delete),
            KeyCode::Char('f') => Some(PickerEvent::CycleViewMode),
            KeyCode::Char('c') => Some(PickerEvent::CopySessionId),
            KeyCode::Char('n') => Some(PickerEvent::NewSession),
            KeyCode::Char('r') => Some(PickerEvent::Refresh),

            KeyCode::Char('y') => Some(PickerEvent::ConfirmAction),
            KeyCode::Char('q') => Some(PickerEvent::Exit),
            KeyCode::Esc => Some(PickerEvent::Exit),

            _ => None,
        }
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
    let total_sessions = sessions.len();
    let items_per_page = 30;
    let total_pages = (total_sessions + items_per_page - 1) / items_per_page;
    let current_page = 1; // Default to page 1 for this renderer

    let header = format!(
        "SESSIONS  │  Page {}/{} │ Showing {}/{}",
        current_page, total_pages, sessions.len(), total_sessions
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

/// Format left panel with pagination state  - displays paginated session list with pagination info
#[allow(dead_code)]
fn format_left_panel_sessions_paginated(sessions: &[SessionInfo], state: &PickerState, _width: u16) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if sessions.is_empty() {
        lines.push("No sessions".yellow().into());
        return lines;
    }

    // Pagination information
    let total_pages = state.pagination.total_pages();
    let current_page = state.pagination.current_page + 1; // Display as 1-indexed
    let showing = sessions.len();

    let header = format!(
        "SESSIONS  │  Page {}/{} │ Showing {}/{}",
        current_page, total_pages, showing, state.sessions.len()
    );
    lines.push(ansi_escape_line(&header).bold());
    lines.push(Line::from(""));

    // Display sessions for current page (3 lines per session + 1 blank line spacing)
    for (page_idx, session) in sessions.iter().enumerate() {
        // Calculate absolute index in the full list
        let abs_idx = state.pagination.page_start() + page_idx;
        let is_selected = state.selected_idx == abs_idx;
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
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

    // Create the SessionPickerOverlay directly, not a StaticOverlay
    let picker_overlay = crate::pager_overlay::SessionPickerOverlay::new(sessions);

    // Wrap it in the Overlay::SessionPicker variant
    Ok(Overlay::SessionPicker(picker_overlay))
}

/// Render the picker view based on current state
pub fn render_picker_view(state: &PickerState) -> Result<Vec<Line<'static>>, String> {
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
