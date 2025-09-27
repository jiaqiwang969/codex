use crate::{
    RolloutRecorder, exec_command::ExecSessionManager,
    mcp_connection_manager::McpConnectionManager, unified_exec::UnifiedExecSessionManager,
    user_notification::UserNotifier,
};
use std::path::PathBuf;
use tokio::sync::Mutex;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: McpConnectionManager,
    pub(crate) session_manager: ExecSessionManager,
    pub(crate) unified_exec_manager: UnifiedExecSessionManager,
    pub(crate) notifier: UserNotifier,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) codex_linux_sandbox_exe: Option<PathBuf>,
    pub(crate) user_shell: crate::shell::Shell,
    pub(crate) show_raw_agent_reasoning: bool,
}
