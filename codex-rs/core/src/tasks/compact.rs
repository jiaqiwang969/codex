use std::sync::Arc;

use async_trait::async_trait;

use crate::{
    codex::{TurnContext, compact},
    protocol::InputItem,
    state::TaskKind,
};

use super::{SessionTask, SessionTaskContext};

#[derive(Clone, Copy, Default)]
pub(crate) struct CompactTask;

#[async_trait]
impl SessionTask for CompactTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Compact
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        sub_id: String,
        input: Vec<InputItem>,
    ) -> Option<String> {
        compact::run_compact_task(session.clone_session(), ctx, sub_id, input).await
    }
}
