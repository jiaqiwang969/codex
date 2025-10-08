//! Agent executor that runs codex with resume-clone in isolated worktrees

use crate::worktree::AgentWorktree;
use crate::{AgentConfig, AgentResult, SessionRecorder};
use anyhow::{Context, Result};
use std::sync::Arc;
use tokio::process::Command;

/// Executes agents with resume-clone
#[derive(Clone)]
pub struct AgentExecutor {
    parent_session: String,
}

impl AgentExecutor {
    /// Create a new agent executor
    pub fn new(parent_session: String) -> Self {
        Self { parent_session }
    }

    /// Execute a single agent in its worktree
    pub(crate) async fn execute(
        &self,
        config: &AgentConfig,
        worktree: &AgentWorktree,
        session_recorder: Arc<SessionRecorder>,
        run_id: &str,
    ) -> Result<AgentResult> {
        // 1. Build prompt
        let prompt = format!(
            r#"
你的角色：{} - {}

基于之前对话中用户的需求，请从你的专业角度实现解决方案。
直接开始编写代码，完成后系统会自动提交。
"#,
            config.name, config.role
        );

        // 2. Execute codex with resume-clone
        let codex_bin = std::env::var("CODEX_BIN").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.npm-global/bin/codex", home)
        });

        // Create a temporary file for session metadata output (use absolute path)
        let project_root = std::env::current_dir().context("Failed to get current directory")?;
        let id_output_path = project_root.join(format!(
            ".tumix/agent-{}-{}-session.json",
            run_id, config.id
        ));
        let id_output_arg = format!("--id-output={}", id_output_path.display());

        // Build command args with --id-output for immediate session info
        let args = vec![
            "exec",
            "--print-rollout-path",
            "--skip-git-repo-check",
            &id_output_arg,
            "--sandbox",
            "danger-full-access",
            "--model",
            "gpt-5-codex-high",
            "resume-clone",
            &self.parent_session,
        ];

        tracing::debug!(
            "Agent {}: Executing command: {} {}",
            config.id,
            codex_bin,
            args.join(" ")
        );

        let output = Command::new(&codex_bin)
            .args(args)
            .arg(&prompt)
            .current_dir(&worktree.path)
            .output()
            .await
            .context(format!("Failed to execute agent {}", config.id))?;

        // Check execution status
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::error!("Agent {} codex execution failed", config.id);
            tracing::error!("  Exit code: {:?}", output.status.code());
            tracing::error!(
                "  Stderr: {}",
                &stderr.chars().take(500).collect::<String>()
            );
            tracing::error!(
                "  Stdout: {}",
                &stdout.chars().take(200).collect::<String>()
            );
            anyhow::bail!(
                "Agent {} execution failed with exit code {:?}:
{}",
                config.id,
                output.status.code(),
                &stderr.chars().take(300).collect::<String>()
            );
        }

        tracing::debug!("Agent {}: Command completed successfully", config.id);

        // 3. Read session metadata from the output file written by codex
        let metadata_content = std::fs::read_to_string(&id_output_path).context(format!(
            "Failed to read session metadata from {}",
            id_output_path.display()
        ))?;

        let metadata: serde_json::Value = serde_json::from_str(&metadata_content)
            .context("Failed to parse session metadata JSON")?;

        let session_id = metadata["session_id"]
            .as_str()
            .context("Missing session_id in metadata")?
            .to_string();

        let jsonl_path = metadata["rollout_path"]
            .as_str()
            .context("Missing rollout_path in metadata")?
            .to_string();

        tracing::debug!(
            "Agent {}: New session {} (JSONL: {})",
            config.id,
            &session_id[..8],
            &jsonl_path
        );

        // Immediately update round1_sessions.json with session info
        session_recorder.record_session_start(&config.id, &session_id, &jsonl_path)?;

        // Clean up temporary metadata file
        let _ = std::fs::remove_file(&id_output_path);

        // 4. Auto-commit changes
        let commit_hash = worktree
            .auto_commit()
            .context("Failed to commit agent work")?;

        Ok(AgentResult {
            agent_id: config.id.clone(),
            session_id,
            commit_hash,
            branch: worktree.branch.clone(),
            jsonl_path,
        })
    }
}
