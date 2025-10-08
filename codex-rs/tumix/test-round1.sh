#!/bin/bash
# TUMIX Round 1 测试脚本

set -e

echo "🚀 TUMIX Round 1 测试"
echo "===================="
echo ""

# 检查参数
if [ $# -lt 1 ]; then
    echo "用法: $0 <parent-session-id>"
    echo ""
    echo "示例:"
    echo "  $0 0199beb3-4c99-78a2-a322-516293137539"
    echo ""
    echo "如何获取 session-id:"
    echo "  1. 在 codex GUI 中与 AI 对话"
    echo "  2. 查看最后的 .jsonl 文件路径"
    echo "  3. 提取文件名中的 UUID 部分"
    exit 1
fi

PARENT_SESSION=$1

echo "📋 配置信息:"
echo "  - Parent Session: ${PARENT_SESSION:0:8}..."
echo "  - Codex Binary: ${CODEX_BIN:-codex}"
echo "  - Working Dir: $(pwd)"
echo ""

# 检查是否在 git 仓库中
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "❌ 错误: 当前目录不是 Git 仓库"
    exit 1
fi

echo "✅ Git 仓库检查通过"
echo ""

# 编译 TUMIX
echo "🔨 编译 TUMIX..."
cd "$(dirname "$0")/.."
cargo build --example tumix-test --release
echo ""

# 运行 TUMIX
echo "🚀 启动 TUMIX Round 1..."
echo "━━━━━━━━━━━━━━━━━━━━"
echo ""

export RUST_LOG=info
./target/release/examples/tumix-test "$PARENT_SESSION"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━"
echo ""

# 显示结果
if [ -f .tumix/round1_sessions.json ]; then
    echo "✅ TUMIX 执行完成！"
    echo ""
    echo "📊 结果摘要:"
    AGENT_COUNT=$(jq length .tumix/round1_sessions.json)
    echo "  - 成功执行: $AGENT_COUNT 个agents"
    echo ""
    echo "📋 Agent列表:"
    jq -r '.[] | "  - Agent \(.agent_id): session=\(.session_id[0:8])..., commit=\(.commit[0:8])..."' .tumix/round1_sessions.json
    echo ""
    echo "🌳 Git分支:"
    git branch --list "round1-agent-*" | head -5
    if [ $(git branch --list "round1-agent-*" | wc -l) -gt 5 ]; then
        echo "  ..."
    fi
    echo ""
    echo "💾 Session列表: .tumix/round1_sessions.json"
else
    echo "❌ 未找到结果文件"
    exit 1
fi
