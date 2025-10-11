# TUMIX Multi-Agent Execution

TUMIX is an experimental orchestration layer that spins up 15 specialized Codex agents in parallel. Each agent works inside its own Git worktree cloned from the active repository, allowing them to explore solutions independently while sharing the full conversation history via `resume-clone`.

## Entry Points

- **CLI**: `codex tumix <SESSION_ID>` resumes an existing Codex conversation and runs the agent swarm from the terminal. The command prints progress logs and a summary with branch names and commits when the run completes.
- **TUI**: `/tumix <task>` launches the same workflow directly from the chat interface. The TUI automatically fetches the active session ID, displays a help card if no task is supplied, and streams status updates back into the transcript. Use `/tumix-stop` (optionally `/tumix-stop <SESSION_ID>`) to request cancellation of in-flight agent runs.

Detailed usage guides live alongside the crate:

- `tumix/README.md` — high-level overview and architecture diagram.
- `tumix/CLI_INTEGRATION.md` — command-line integration details.
- `tumix/GUI_INTEGRATION.md` — slash command behaviour and UX.

## Execution Flow

1. **Meta prompt**: A coordinating agent analyzes the task and produces 15 role definitions (system architect, core algorithm engineer, QA, etc.).
2. **Workspace setup**: The runner provisions one Git worktree per agent so their commits never collide.
3. **Agent runs**: For each role, `codex exec --print-rollout-path --skip-git-repo-check --sandbox danger-full-access --model gpt-5-codex-high resume-clone …` is invoked with the shared conversation history and role-specific instructions.
4. **Result collation**: The runner records the session ID, branch name, and commit hash for every agent, then persists the list to `.tumix/round1_sessions.json`.

## Output Artifacts

- `.tumix/round1_sessions.json` — manifest of agents, sessions, and commits for later rounds.
- `round1-agent-XX` branches — one branch per agent rooted at the parent session's tip.
- Agent commits — each agent auto-commits its work within its branch.

## Requirements and Limitations

- Runs inside an initialized Git repository with a clean base branch.
- Requires Codex builds that ship the `codex tumix` subcommand (or the `/tumix` slash command in the TUI).
- Designed for sandboxed environments; commands automatically pass `--skip-git-repo-check` and related flags needed by Codex automation.
- Round-one orchestration is implemented today; multi-round iteration is slated for future work.

## Troubleshooting

- **No active session**: The slash command warns when the GUI session has not been established yet. Start a normal conversation first.
- **Missing results file**: Check that the workspace is writable and that the `codex` binary is discoverable via `$PATH` or the `CODEX_BIN` environment variable.
- **Partial agent success**: Inspect the per-agent branches to review individual histories, then re-run TUMIX once the underlying issue is resolved.

## Further Reading

- `tumix/IMPLEMENTATION.md` — internal architecture and async execution details.
- `tumix/FINAL_FIX.md` — post-integration fixes, argument defaults, and UX polish.
- `codex-rs/diagnose-tumix.sh` — helper script to collect diagnostics from a failing TUMIX run.
