import { execaCommand } from 'execa';
import chalk from 'chalk';

export async function launchCodexRaw({ codexCmd = 'codex', workingDir }) {
  try {
    const child = execaCommand(codexCmd, { stdio: 'inherit', shell: true, cwd: workingDir || process.cwd() });
    await child;
  } catch (err) {
    console.error(chalk.red('无法启动 Codex：'), err?.shortMessage || err?.message || err);
  }
}
// Simple launcher only; resume-by-id handled by caller via command string
