//! Simple test binary for TUMIX

use codex_tumix::{self, ProgressCallback};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing for CLI runs
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Get parent session and optional prompt from command line
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: tumix-test <parent-session-id> [optional prompt...]");
        eprintln!("Example: tumix-test 0199beb3-4c99-78a2-a322-516293137539");
        std::process::exit(1);
    }

    let parent_session = args[1].clone();
    let user_prompt = if args.len() > 2 {
        Some(args[2..].join(" "))
    } else {
        None
    };

    let session_preview: String = parent_session.chars().take(8).collect();
    println!("🚀 Starting TUMIX with parent session: {session_preview}");

    let progress_callback: ProgressCallback = Box::new(|msg| println!("{msg}"));

    // Run TUMIX with optional prompt and progress reporting
    match codex_tumix::run_tumix(parent_session, user_prompt, Some(progress_callback)).await {
        Ok(result) => {
            println!("\n✨ TUMIX Round 1 完成！");
            println!("成功执行：{} 个专家", result.agents.len());
            println!("\n📋 Agent结果：");
            for agent in &result.agents {
                let session_preview: String = agent.session_id.chars().take(8).collect();
                let commit_preview: String = agent.commit_hash.chars().take(8).collect();
                println!(
                    "  - Agent {}: session={}, commit={}",
                    agent.agent_id, session_preview, commit_preview
                );
            }
            println!("\n💾 Session列表已保存: .tumix/round1_sessions.json");
        }
        Err(e) => {
            eprintln!("❌ TUMIX执行失败: {e}");
            std::process::exit(1);
        }
    }

    Ok(())
}
