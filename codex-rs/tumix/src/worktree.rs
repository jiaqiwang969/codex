//! Git worktree management for isolated agent execution

use anyhow::{Context, Result};
use git2::Repository;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages Git worktrees for agent isolation
pub struct WorktreeManager {
    repo: Repository,
    worktrees_root: PathBuf,
    run_id: String,
}

impl WorktreeManager {
    /// Create a new worktree manager with unique run ID
    pub fn new(repo_path: &Path, run_id: &str) -> Result<Self> {
        let repo = Repository::open(repo_path).context("Failed to open Git repository")?;

        let worktrees_root = repo_path.join(format!(".tumix/worktrees/{}", run_id));
        std::fs::create_dir_all(&worktrees_root).context("Failed to create worktrees directory")?;

        Ok(Self {
            repo,
            worktrees_root,
            run_id: run_id.to_string(),
        })
    }

    /// Create an isolated worktree for an agent
    pub fn create(&self, agent_id: &str) -> Result<AgentWorktree> {
        let branch_name = format!("round1-{}-agent-{}", self.run_id, agent_id);
        let worktree_path = self.worktrees_root.join(format!("agent-{}", agent_id));

        // Clean up existing worktree
        if worktree_path.exists() {
            tracing::debug!("Removing existing worktree: {}", worktree_path.display());
            self.remove_worktree(&worktree_path)?;
        }

        // Create new worktree based on current branch (HEAD)
        let repo_root = self
            .repo
            .path()
            .parent()
            .context("Failed to get repo root")?;

        tracing::debug!(
            "Creating worktree for agent {}: {}",
            agent_id,
            worktree_path.display()
        );

        let output = Command::new("git")
            .args(["worktree", "add", "-b", &branch_name])
            .arg(&worktree_path)
            .arg("HEAD")
            .current_dir(repo_root)
            .output()
            .context("Failed to execute git worktree command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            tracing::error!("Git worktree creation failed for agent {}", agent_id);
            tracing::error!("  Exit code: {:?}", output.status.code());
            tracing::error!("  Stdout: {}", stdout);
            tracing::error!("  Stderr: {}", stderr);
            anyhow::bail!(
                "Failed to create worktree for agent {}: {}",
                agent_id,
                stderr
            );
        }

        tracing::info!(
            "✅ Created worktree for agent {}: {}",
            agent_id,
            worktree_path.display()
        );

        Ok(AgentWorktree {
            path: worktree_path,
            branch: branch_name,
            agent_id: agent_id.to_string(),
        })
    }

    /// Remove a worktree
    fn remove_worktree(&self, path: &Path) -> Result<()> {
        // Git worktree remove
        let _ = Command::new("git")
            .args(["worktree", "remove", "-f"])
            .arg(path)
            .output();

        // Force remove directory if still exists
        if path.exists() {
            std::fs::remove_dir_all(path).context("Failed to remove worktree directory")?;
        }

        Ok(())
    }
}

/// An isolated worktree for agent execution
pub struct AgentWorktree {
    pub path: PathBuf,
    pub branch: String,
    pub agent_id: String,
}

impl AgentWorktree {
    /// Auto-commit all changes in the worktree
    pub fn auto_commit(&self) -> Result<String> {
        let repo = Repository::open(&self.path).context("Failed to open worktree repository")?;

        // git add .
        let mut index = repo.index().context("Failed to get index")?;
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .context("Failed to stage files")?;
        index.write().context("Failed to write index")?;

        // Check if there are changes
        let tree_id = index.write_tree().context("Failed to write tree")?;
        let tree = repo.find_tree(tree_id).context("Failed to find tree")?;

        // Get parent commit
        let parent_commit = repo
            .head()
            .context("Failed to get HEAD")?
            .peel_to_commit()
            .context("Failed to peel to commit")?;

        // Check if tree changed
        if tree.id() == parent_commit.tree_id() {
            tracing::debug!("Agent {}: No changes to commit", self.agent_id);
            return Ok(parent_commit.id().to_string());
        }

        // Create commit
        let signature = repo.signature().context("Failed to create signature")?;

        let commit_msg = format!(
            "Round 1 - Agent {}\n\n🤖 Generated with TUMIX\n\nCo-Authored-By: Agent {} <agent{}@tumix.local>",
            self.agent_id, self.agent_id, self.agent_id
        );

        let commit_id = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                &commit_msg,
                &tree,
                &[&parent_commit],
            )
            .context("Failed to create commit")?;

        tracing::debug!("Agent {}: Committed {}", self.agent_id, commit_id);

        Ok(commit_id.to_string())
    }
}
