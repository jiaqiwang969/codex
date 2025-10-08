# TUMIX - Multi-Agent Parallel Execution Framework

TUMIX enables running 15 specialized agents in parallel, each working in isolated Git worktrees with cloned conversation contexts via `resume-clone`.

## Architecture

```
Parent Session (GUI)
    ↓ resume-clone
    ├── Agent 01 → worktree + session
    ├── Agent 02 → worktree + session
    └── ...
        └── Agent 15 → worktree + session
```

## Features

- **Zero Configuration**: Just `/tumix` in codex GUI
- **Automatic Context**: Uses `resume-clone` to inherit conversation history
- **Isolated Execution**: Each agent in separate Git worktree
- **Parallel Processing**: 15 agents run concurrently via tokio
- **Session Tracking**: Saves all session IDs for Round 2+

## Usage

### In Codex GUI (Future)

```
User: 帮我实现一个Rust自动微分库
Assistant: [discusses requirements]
User: /tumix
Assistant: ✨ TUMIX Round 1 完成！15个专家成功执行
```

### Testing with CLI

```bash
# Set codex binary path (if not in PATH)
export CODEX_BIN=/path/to/codex

# Run test with a session ID
cd /path/to/your/git/repo
cargo run --example tumix-test -- <parent-session-id>
```

## Output

### Git Branches
```
main
 ├── round1-agent-01  (系统架构师)
 ├── round1-agent-02  (核心算法工程师)
 └── ...
```

### Session List
`.tumix/round1_sessions.json`:
```json
[
  {
    "agent_id": "01",
    "session_id": "0199beb3-4c99-78a2-a322-516293137539",
    "commit": "a1b2c3d4...",
    "branch": "round1-agent-01"
  },
  ...
]
```

## Implementation

### Core Modules

- **`lib.rs`**: Main entry point (`run_tumix`)
- **`meta.rs`**: Meta-agent generates 15 agent configs
- **`worktree.rs`**: Git worktree management
- **`executor.rs`**: Agent execution with `resume-clone`

### Agent Workflow

1. Meta-agent analyzes conversation → generates 15 agent configs
2. Create 15 isolated worktrees (based on `main`)
3. Execute 15 agents in parallel:
   - `codex exec resume-clone <parent-session>`
   - Agent writes code in its worktree
   - Auto-commit changes
4. Extract session IDs from stderr
5. Save session list to `.tumix/round1_sessions.json`

## Future: Round 2+

```rust
// Read session list from Round 1
let round1_sessions = load_sessions(".tumix/round1_sessions.json")?;

// For each agent, resume from its Round 1 session
for (agent_id, session) in round1_sessions {
    run_agent_round2(agent_id, session.session_id).await?;
}
```

## Environment Variables

- `CODEX_BIN`: Path to codex binary (default: `"codex"`)

## Requirements

- Rust 2024 edition
- Git repository
- Codex with `resume-clone` support

## License

Same as codex-rs parent project.
