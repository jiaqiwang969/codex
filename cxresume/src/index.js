import fs from 'node:fs';
import path from 'node:path';
import chalk from 'chalk';
import { fileURLToPath } from 'node:url';
import { loadConfig, resolveLogsRoot } from './utils/config.js';
import { pickSessionInteractively, showMessage, renderPreview } from './ui.js';
import { pickSessionSplitTUI } from './tui/splitPicker.js';
import { parseSessionFile } from './utils/parser.js';
import { listSessionFiles } from './utils/sessionFinder.js';
import { searchSessions } from './utils/search.js';
import { launchCodexRaw } from './utils/launch.js';
import { filterSessionsByCwd, extractSessionMetaQuick } from './utils/metaQuick.js';
import { ensureWorkspaceSessionIds, findSessionsByIds, createWorkspaceSessionAndRecord, readWorkspaceSessionIds } from './utils/workspaceSessions.js';
// dynamic keep is not used in full-history compression mode

function shellQuoteSingleArg(s) {
  if (s === '') return "''";
  return "'" + String(s).replace(/'/g, "'\\''") + "'";
}

function buildResumeCommand(baseCmd, sessionId, extraArgs = '') {
  const parts = [
    baseCmd,
    'resume',
    shellQuoteSingleArg(sessionId),
    (extraArgs || '').trim()
  ].filter(Boolean);
  return parts.join(' ');
}

function parseArgs(argv) {
  const args = { _: [] };
  for (let i = 2; i < argv.length; i++) {
    const a = argv[i];
    if (!a.startsWith('-')) { args._.push(a); continue; }
    const [k, v] = a.includes('=') ? a.split('=') : [a, undefined];
    switch (k) {
      case '-h':
      case '--help': args.help = true; break;
      case '-v':
      case '--version': args.version = true; break;
      case '--list': args.list = true; break;
      case '--open': args.open = argv[++i] || v; break;
      case '--root': args.root = argv[++i] || v; break;
      case '--codex': args.codex = argv[++i] || v; break;
      case '--search': args.search = argv[++i] || v; break;
      case '--preview': args.preview = true; break;
      case '--no-preview': args.preview = false; break;
      // injection is default and not configurable via CLI anymore
      case '--hide': {
        // Collect subsequent non-option tokens as hide options
        const valid = new Set(['tool','thinking','user','assistant','system']);
        const vals = [];
        let j = i + 1;
        while (j < argv.length && !String(argv[j]).startsWith('-')) {
          const tok = String(argv[j]);
          if (valid.has(tok)) vals.push(tok);
          else break;
          j++;
        }
        args.hide = vals.length ? vals : ['tool','thinking'];
        i = j - 1;
        break;
      }
      case '-y':
      case '--yes': args.yes = true; break;
      case '-n':
      case '--new': args.newSession = true; break;
      case '-l':
      case '--latest': args.resumeLatest = true; break;
      case '--legacy-ui': args.legacyUI = true; break;
      case '--print': args.print = true; break;
      case '--no-launch': args.noLaunch = true; break;
      case '--debug': args.debug = true; break;
      default:
        console.warn(chalk.yellow(`Unknown option: ${a}`));
    }
  }
  return args;
}

function showHelp() {
  const __dirname = path.dirname(fileURLToPath(import.meta.url));
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
  console.log(`\n${chalk.cyan('cxresume')} v${pkg.version}\n`);
  console.log('Resume Codex sessions from ~/.codex/sessions');
  console.log('\nUsage:');
  console.log('  cxresume               # interactive session picker');
  console.log('  cxresume --list        # list recent session files');
  console.log('  cxresume --open <file> # open a specific session file');
  console.log('  cxresume cwd           # resume sessions recorded for current workspace');
  console.log('  cxresume .             # filter sessions by current working directory (if available)');
  console.log('\nOptions:');
  console.log('  --root <dir>           Override sessions root (default: ~/.codex/sessions)');
  console.log('  --codex <cmd>          Codex launch command (default: "codex")');
  // console.log('  --keep-last <n>        Keep last N messages verbatim (default: 8)');
  console.log('  --search <text>        Content search, then pick from matches');
  console.log('  --hide [types...]      Hide types in preview: tool thinking user assistant system (default: tool thinking)');
  // injection is inline by default; advanced injection flags removed for simplicity
  console.log('  --legacy-ui            Use legacy single-prompt selector (no split view)');
  console.log('  --print                Only print the command and exit');
  console.log('  --no-launch            Do not launch Codex');
  console.log('  -n, --new              (with "cwd") create, record, and resume a new workspace session');
  console.log('  -l, --latest           (with "cwd") resume the most recent recorded workspace session');
  console.log('  --debug                Print extra diagnostics');
  console.log('  -h, --help             Show help');
  console.log('  -v, --version          Show version');
}

async function main() {
  const args = parseArgs(process.argv);
  const positional = Array.isArray(args._) ? [...args._] : [];
  const workspaceMode = positional.includes('cwd');
  if (workspaceMode) {
    args._ = positional.filter(x => x !== 'cwd');
  } else {
    args._ = positional;
  }
  if (args.help) return showHelp();
  if (args.version) {
    const __dirname = path.dirname(fileURLToPath(import.meta.url));
    const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'package.json'), 'utf8'));
    console.log(pkg.version);
    return;
  }

  const cfg = await loadConfig({ overrideCodexCmd: args.codex, overrideRoot: args.root });
  const root = resolveLogsRoot(cfg);

  if (args.list) {
    const files = await listSessionFiles(root);
    if (!files.length) {
      console.log(chalk.yellow('No session files found.'), `Root: ${root}`);
      return;
    }
    console.log(chalk.cyan(`Found ${files.length} sessions under ${root}`));
    for (const f of files.slice(0, 100)) {
      console.log(`- ${f.rel} ${chalk.gray(`(${new Date(f.mtime).toLocaleString()})`)}`);
    }
    if (files.length > 100) console.log(chalk.gray(`... and ${files.length - 100} more`));
    return;
  }

  if (workspaceMode && args.newSession && args.resumeLatest) {
    console.error(chalk.red('选项 -n 和 -l 不能同时使用。'));
    return;
  }

  async function resumeSessionById(sessionId, { extraArgs = '', workingDir } = {}) {
    const cmd = buildResumeCommand(cfg.codexCmd, sessionId, extraArgs);
    if (args.print) {
      console.log(cmd);
      return;
    }
    if (args.noLaunch) {
      await showMessage('已生成命令但未启动（--no-launch）。');
      return;
    }
    await launchCodexRaw({ codexCmd: cmd, workingDir: workingDir || process.cwd() });
  }

  async function createAndResumeWorkspaceSession() {
    try {
      const created = await createWorkspaceSessionAndRecord(process.cwd(), { codexCmd: cfg.codexCmd });
      await resumeSessionById(created.id);
    } catch (err) {
      const msg = err?.shortMessage || err?.message || err;
      console.error(chalk.red(`创建新的工作区会话失败：${msg}`));
      if (args.debug && err?.stack) console.error(chalk.gray(err.stack));
    }
  }

  let workspaceSessionIds = [];
  let workspaceSessions = [];
  let workspacePathsSet = null;
  if (workspaceMode) {
    const cwd = process.cwd();
    try {
      if (args.newSession) {
        workspaceSessionIds = await readWorkspaceSessionIds(cwd);
      } else {
        workspaceSessionIds = await ensureWorkspaceSessionIds(cwd, { codexCmd: cfg.codexCmd });
      }
      if (!args.newSession) {
        let lookup = await findSessionsByIds(root, workspaceSessionIds);
        if ((!lookup.files || !lookup.files.length) && workspaceSessionIds.length) {
          await new Promise(resolve => setTimeout(resolve, 300));
          lookup = await findSessionsByIds(root, workspaceSessionIds);
        }
        const { files = [], missingIds = [] } = lookup;
        workspaceSessions = files;
        workspacePathsSet = new Set(files.map(f => path.resolve(f.path)));
        if (missingIds.length && args.debug) {
          console.error(chalk.gray(`Missing session files for ids: ${missingIds.join(', ')}`));
        }
      }
    } catch (err) {
      if (args.debug) console.error(chalk.red(`Workspace session setup failed: ${err?.message || err}`));
    }
  }

  if (workspaceMode && args.newSession) {
    await createAndResumeWorkspaceSession();
    return;
  }

  if (workspaceMode && args.resumeLatest) {
    const latestId = workspaceSessionIds.at(-1);
    if (!latestId) {
      console.log(chalk.yellow('当前目录下暂无已记录的 session。'));
      return;
    }
    await resumeSessionById(latestId);
    return;
  }

  // Handle current dir filter shorthand '.' in positional args
  const hadDot = (args._ || []).includes('.');
  const currentDirOnly = !workspaceMode && hadDot;
  if (hadDot) args._ = args._.filter(x => x !== '.');

  // Find session: via --search, --open or interactive
  let targetFile = args.open;
  let tuiResult = null;
  if (args.search) {
    const results = await searchSessions(root, args.search);
    const scopedResults = workspaceMode && workspacePathsSet
      ? results.filter(r => workspacePathsSet.has(path.resolve(r.path)))
      : results;
    if (!scopedResults.length) {
      const baseMsg = `No matches for "${args.search}" under ${root}`;
      if (workspaceMode) console.log(chalk.yellow(`${baseMsg} (workspace sessions only).`));
      else console.log(chalk.yellow(baseMsg));
      return;
    }
    // Use split TUI with prefiltered results
    tuiResult = await pickSessionSplitTUI(root, scopedResults, { hide: args.hide, currentDirOnly: false, workspaceMode });
    if (!tuiResult) return;
    if (tuiResult.action === 'startNew') {
      const cmd = [cfg.codexCmd, (tuiResult.extraArgs || '').trim()].filter(Boolean).join(' ');
      await launchCodexRaw({ codexCmd: cmd, workingDir: tuiResult.workingDir || process.cwd() });
      return;
    }
    if (tuiResult.action === 'workspaceCreate') {
      await createAndResumeWorkspaceSession();
      return;
    }
    targetFile = tuiResult.path;
  } else if (!targetFile) {
    let preset = null;
    if (workspaceMode) {
      preset = workspaceSessions.slice().sort((a, b) => b.mtime - a.mtime);
      if (!preset.length) {
        if (workspaceSessionIds.length) {
          console.log(chalk.yellow('未找到对应 session 文件，请稍候后再试或检查 ~/.codex/sessions。'));
        } else {
          console.log(chalk.yellow('当前目录下暂无已记录的 session。'));
        }
        return;
      }
    } else if (currentDirOnly) {
      try {
        preset = await filterSessionsByCwd(root, process.cwd());
        if (!preset.length) console.log(chalk.gray('No sessions matched current directory; showing all.'));
      } catch {}
    }
    const pickerOptions = { hide: args.hide, currentDirOnly: workspaceMode ? false : currentDirOnly, workspaceMode };
    const choice = args.legacyUI
      ? await pickSessionInteractively(root, preset)
      : await pickSessionSplitTUI(root, preset, pickerOptions);
    if (!choice) return; // user aborted
    if (choice.action === 'startNew') {
      const cmd = [cfg.codexCmd, (choice.extraArgs || '').trim()].filter(Boolean).join(' ');
      await launchCodexRaw({ codexCmd: cmd, workingDir: choice.workingDir || process.cwd() });
      return;
    }
    if (choice.action === 'workspaceCreate') {
      await createAndResumeWorkspaceSession();
      return;
    }
    tuiResult = choice;
    targetFile = choice.path;
  } else {
    // allow shorthand relative segments under root
    const abs = path.isAbsolute(targetFile) ? targetFile : path.join(root, targetFile);
    if (fs.existsSync(abs)) targetFile = abs;
    if (workspaceMode && workspacePathsSet && !workspacePathsSet.has(path.resolve(targetFile))) {
      console.error(chalk.red('当前目录未记录该 session；请使用 cxresume cwd 选择已记录的 session。'));
      return;
    }
  }

  if (!fs.existsSync(targetFile)) {
    console.error(chalk.red(`File not found: ${targetFile}`));
    process.exit(2);
  }

  // Extract session id quickly
  if (args.debug) console.error(chalk.gray(`Extracting session id from ${targetFile} ...`));
  let quickMeta = null;
  try { quickMeta = await extractSessionMetaQuick(targetFile); } catch {}
  const sessionId = quickMeta?.id || path.relative(root, targetFile);
  if (!quickMeta?.id) {
    console.log(chalk.yellow(`Warning: session id not found in meta; using relative path as id: ${sessionId}`));
  }

  // Optional preview (best-effort)
  const wantPreview = args.preview !== undefined ? args.preview : cfg.preview;
  if (wantPreview && !args.print) {
    try {
      const { messages } = await parseSessionFile(targetFile);
      if (messages?.length) {
        console.log(chalk.magenta('\nPreview of recent dialog:'));
        console.log(renderPreview({ messages, max: 5, query: args.search }));
      }
    } catch {}
  }

  // Build command: codex resume <sessionId> [extraArgs]
  const extraArgs = (tuiResult?.extraArgs || '').trim();
  await resumeSessionById(sessionId, { extraArgs, workingDir: tuiResult?.workingDir || process.cwd() });
}

main();
