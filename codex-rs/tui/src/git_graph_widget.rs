use crate::pager_overlay::Overlay;
use codex_ansi_escape::ansi_escape_line;
use ratatui::{style::Stylize, text::Line};
use std::{path::Path, process::Command};

// Prefer the embedded git-graph library (round Unicode style) when available.
// This produces rounded connectors (╭╮╯╰) and colored branches. If it fails
// for any reason, we fall back to `git log --graph`.
#[allow(unused_imports)]
use {
    git_graph::config::get_model_name,
    git_graph::get_repo,
    git_graph::graph::GitGraph,
    git_graph::print::format::CommitFormat,
    git_graph::print::unicode::print_unicode,
    git_graph::settings::{
        BranchOrder, BranchSettings, BranchSettingsDef, Characters, MergePatterns, Settings,
    },
};

/// Convert ASCII art to round/Unicode style
fn convert_to_round_style(line: &str) -> String {
    let mut result = line.to_string();

    // Replace ASCII graph characters with round Unicode equivalents
    result = result.replace('*', "●"); // Replace asterisk with bullet
    result = result.replace('|', "│"); // Replace pipe with box drawing
    result = result.replace('\\', "╲"); // Replace backslash with diagonal
    result = result.replace('/', "╱"); // Replace forward slash with diagonal
    result = result.replace('-', "─"); // Replace dash with horizontal line

    // Handle merge patterns
    result = result.replace("│╲", "│╲");
    result = result.replace("│╱", "│╱");
    result = result.replace("╲│", "╲│");
    result = result.replace("╱│", "╱│");

    // Make some visual improvements
    result = result.replace("  ●", "  ●"); // Keep bullet spacing
    result = result.replace("● ", "● "); // Keep bullet spacing

    result
}

/// Generate git graph lines for display in the TUI overlay.
pub fn generate_git_graph<P: AsRef<Path>>(repo_path: P) -> Result<Vec<Line<'static>>, String> {
    // First try: high-quality round Unicode graph via git-graph library.
    if let Ok(lines) = generate_with_git_graph(repo_path.as_ref()) {
        return Ok(lines);
    }

    // Fallback: use `git log --graph` (ASCII) and do a best-effort conversion
    // to Unicode line drawing characters.
    // Try git log with detailed formatting and more commits for scrolling
    // Show the full history (no artificial commit limit); the pager overlay
    // handles scrolling efficiently. This avoids surprising truncation in
    // larger repos where 50 commits isn't enough.
    let output = Command::new("git")
        .args(&[
            "log",
            "--graph",
            "--pretty=format:%C(auto)%h %s %C(green)(%cr) %C(bold blue)<%an>%C(reset)%C(auto)%d",
            "--all",
            "--color=always",
            "--abbrev-commit",
        ])
        .current_dir(&repo_path)
        .output()
        .map_err(|e| format!("Failed to execute git log: {}", e))?;

    if !output.status.success() {
        // Fallback to simpler git log if the above fails
        let fallback_output = Command::new("git")
            .args(&["log", "--graph", "--oneline", "--all", "--color=always"])
            .current_dir(&repo_path)
            .output()
            .map_err(|e| format!("Failed to execute fallback git log: {}", e))?;

        if !fallback_output.status.success() {
            return Err(format!(
                "Git command failed: {}",
                String::from_utf8_lossy(&fallback_output.stderr)
            ));
        }

        let output_str = String::from_utf8_lossy(&fallback_output.stdout);
        return if output_str.trim().is_empty() {
            Ok(vec!["No git history found.".dim().into()])
        } else {
            // Convert to round style and process ANSI
            let lines: Vec<Line<'static>> = output_str
                .lines()
                .map(|line| {
                    let round_line = convert_to_round_style(line);
                    ansi_escape_line(&round_line)
                })
                .collect();
            Ok(lines)
        };
    }

    let output_str = String::from_utf8_lossy(&output.stdout);

    if output_str.trim().is_empty() {
        Ok(vec!["No git history found.".dim().into()])
    } else {
        // Convert each line to round style, then process ANSI escapes
        let lines: Vec<Line<'static>> = output_str
            .lines()
            .map(|line| {
                let round_line = convert_to_round_style(line);
                ansi_escape_line(&round_line)
            })
            .collect();
        Ok(lines)
    }
}

/// Create a new git graph overlay for the TUI with enhanced title.
pub fn create_git_graph_overlay<P: AsRef<Path>>(repo_path: P) -> Result<Overlay, String> {
    let lines = generate_git_graph(repo_path)?;
    Ok(Overlay::new_static_with_title_no_wrap(
        lines,
        "G I T   G R A P H   │   j/k:scroll   q/Esc:close   │   C t r l + G".to_string(),
    ))
}

// Build lines using the embedded git-graph library with a "round" style.
fn generate_with_git_graph<P: AsRef<Path>>(repo_path: P) -> Result<Vec<Line<'static>>, String> {
    // Discover the repository; allow owner validation to be skipped to avoid
    // platform-specific errors in embedded environments.
    let repo = get_repo(repo_path, true).map_err(|e| format!("libgit2 error: {}", e.message()))?;

    // Load model preference if present; otherwise use a reasonable default.
    let model_name = get_model_name(&repo, "git-graph.toml").unwrap_or(None);
    let model_def = match model_name.as_deref() {
        Some("git-flow") => BranchSettingsDef::git_flow(),
        Some("simple") => BranchSettingsDef::simple(),
        Some("none") => BranchSettingsDef::none(),
        _ => BranchSettingsDef::simple(),
    };
    let branches = BranchSettings::from(model_def).map_err(|e| format!("settings error: {}", e))?;

    // Use rounded characters, include remotes, and colored output like `--all --color`.
    let settings = Settings {
        reverse_commit_order: false,
        debug: false,
        compact: true,
        colored: true,
        include_remote: true,
        // Compact commit summary similar to `--oneline`.
        format: CommitFormat::Short,
        // Let our TUI pager handle wrapping.
        wrapping: None,
        characters: Characters::round(),
        branch_order: BranchOrder::ShortestFirst(true),
        branches,
        merge_patterns: MergePatterns::default(),
    };

    // No artificial limit: let the pager scroll the full history.
    let graph = GitGraph::new(repo, &settings, None)?;
    let (g_lines, t_lines, _indices) = print_unicode(&graph, &settings)?;

    // Join graph and text columns, then parse ANSI into ratatui Line.
    let lines: Vec<Line<'static>> = g_lines
        .into_iter()
        .zip(t_lines.into_iter())
        .map(|(g, t)| ansi_escape_line(&format!(" {g}  {t}")))
        .collect();
    Ok(lines)
}
