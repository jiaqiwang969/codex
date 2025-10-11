use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use anyhow::{Result, anyhow};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct RunDescriptor {
    pub session_id: String,
    pub run_id: String,
}

struct RunEntry {
    run_id: String,
    token: CancellationToken,
}

static RUNS: LazyLock<Mutex<HashMap<String, RunEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Guard that removes the run from the registry when dropped.
pub(crate) struct RunGuard {
    descriptor: RunDescriptor,
    token: CancellationToken,
}

impl RunGuard {
    pub(crate) fn token(&self) -> CancellationToken {
        self.token.clone()
    }
}

impl Drop for RunGuard {
    fn drop(&mut self) {
        let mut runs = RUNS.lock().unwrap();
        runs.remove(&self.descriptor.session_id);
    }
}

pub(crate) fn register_run(session_id: &str, run_id: &str) -> Result<RunGuard> {
    let mut runs = RUNS.lock().unwrap();
    if runs.contains_key(session_id) {
        return Err(anyhow!(
            "TUMIX run already in progress for session {}",
            session_id
        ));
    }

    let token = CancellationToken::new();
    runs.insert(
        session_id.to_string(),
        RunEntry {
            run_id: run_id.to_string(),
            token: token.clone(),
        },
    );

    Ok(RunGuard {
        descriptor: RunDescriptor {
            session_id: session_id.to_string(),
            run_id: run_id.to_string(),
        },
        token,
    })
}

pub fn cancel_session(session_id: &str) -> Option<RunDescriptor> {
    let (token, descriptor) = {
        let runs = RUNS.lock().unwrap();
        runs.get(session_id).map(|entry| {
            (
                entry.token.clone(),
                RunDescriptor {
                    session_id: session_id.to_string(),
                    run_id: entry.run_id.clone(),
                },
            )
        })
    }?;

    token.cancel();
    Some(descriptor)
}

pub fn cancel_all() -> Vec<RunDescriptor> {
    let entries: Vec<_> = {
        let runs = RUNS.lock().unwrap();
        runs.iter()
            .map(|(session_id, entry)| {
                (
                    entry.token.clone(),
                    RunDescriptor {
                        session_id: session_id.clone(),
                        run_id: entry.run_id.clone(),
                    },
                )
            })
            .collect()
    };

    for (token, _) in &entries {
        token.cancel();
    }

    entries.into_iter().map(|(_, desc)| desc).collect()
}
