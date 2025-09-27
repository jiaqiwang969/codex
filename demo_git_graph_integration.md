# Git Graph Integration for Codex TUI

## 集成完成 ✅

我已经成功为 Codex TUI 添加了 git graph 可视化功能。以下是实现的功能：

### 新功能
- **快捷键**: `Ctrl+G` 显示当前目录的 git commit 可视化图
- **界面集成**: 作为全屏 overlay 显示，与现有的 `Ctrl+T` (transcript) 功能保持一致
- **错误处理**: 如果不在 git 仓库中或 git 命令失败，会显示友好的错误信息
- **帮助提示**: 在快捷键帮助 (`?` 键) 中显示 git graph 快捷键

### 实现细节

#### 1. 新文件: `codex-rs/tui/src/git_graph_widget.rs`
- `generate_git_graph()`: 生成 git 历史图
- `create_git_graph_overlay()`: 创建 TUI overlay
- 使用 `git log --graph` 命令生成彩色的 commit 历史
- 支持回退到简单格式如果高级格式失败

#### 2. 修改文件: `codex-rs/tui/src/app.rs`
- 添加 `Ctrl+G` 键处理逻辑
- 集成错误处理，显示友好的错误消息
- 使用现有的 overlay 系统

#### 3. 修改文件: `codex-rs/tui/src/lib.rs`
- 添加 `git_graph_widget` 模块

#### 4. 修改文件: `codex-rs/tui/src/bottom_pane/footer.rs`
- 添加 `ShowGitGraph` 快捷键 ID
- 在快捷键帮助中显示 `ctrl + g to view git graph`

### Git 命令格式

使用了两种 git log 格式：

1. **主要格式** (更漂亮):
```bash
git log --graph --pretty=format:"%C(auto)%h %s %C(green)(%cr) %C(bold blue)<%an>%C(reset)%C(auto)%d" --all --color=always --abbrev-commit -20
```

2. **回退格式** (简单):
```bash
git log --graph --oneline --all --color=always -10
```

### 测试验证

已验证功能在当前 git 仓库中正常工作：
- ✅ Git 命令执行正常
- ✅ 生成彩色的 commit 图
- ✅ 处理 ANSI 转义序列
- ✅ 错误处理机制

### 使用方法

1. 在 codex TUI 中
2. 按 `Ctrl+G` 
3. 查看当前目录的 git commit 历史图
4. 按 `Esc` 或 `q` 退出 overlay

### 预期输出示例

```
* 550a5616 clippy (7 hours ago) <easong-openai> (origin/easong/remote-tasks)
* 534eccf4 feedback (8 hours ago) <easong-openai>
* 04d9bef9 Update codex-rs/backend-client/src/types.rs (9 hours ago) <easong-openai>
* 75a898ae Update codex-rs/backend-client/src/client.rs (9 hours ago) <easong-openai>
* 6d1c79f1 CI (13 hours ago) <easong-openai>
*   c75e0b2b merge (13 hours ago) <easong-openai>
|\  
* | 38cf7c46 make sure preflight is clean (14 hours ago) <easong-openai>
* | 134ab441 CI? (14 hours ago) <easong-openai>
```

### 与现有功能的一致性

- 使用相同的 overlay 系统 (`Overlay::new_static_with_title`)
- 遵循相同的键处理模式
- 集成到现有的快捷键帮助系统
- 使用 ratatui 的 Stylize trait 保持代码风格一致

这个集成为用户提供了一个便捷的方式来查看当前项目的 git 历史，增强了 Codex 的开发体验。