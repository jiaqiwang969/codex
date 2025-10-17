import blessedPkg from 'blessed';
const blessed = blessedPkg;
import dayjs from 'dayjs';
import path from 'node:path';
import clipboard from 'clipboardy';
import fs from 'node:fs/promises';
import chalk from 'chalk';
import stringWidth from 'string-width';
import { listSessionFiles } from '../utils/sessionFinder.js';
import { parseSessionFile } from '../utils/parser.js';
import { extractSessionMetaQuick } from '../utils/metaQuick.js';
import { selectRecentDialogMessages, formatPreviewLines, formatPreviewBlocks } from '../utils/preview.js';

function safeString(s) { return typeof s === 'string' ? s : ''; }

// Terminal-like theme colors inspired by codex_recovery_1.html
const THEME = {
  gray: '#808080',
  green: '#5af78e',
  yellow: '#f3f99d',
  orange: '#ff9500',
  red: '#ff6ac1',
  purple: '#bf5af2',
  blue: '#6ac8ff',
  cyan: '#5fbeaa',
};

function formatRelativeAge(date) {
  try {
    const d = typeof date === 'string' || typeof date === 'number' ? new Date(date) : date;
    const now = Date.now();
    const diff = Math.max(0, now - d.getTime());
    const s = Math.floor(diff / 1000);
    if (s < 60) return `${s}s ago`;
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    const days = Math.floor(h / 24);
    if (days < 7) return `${days}d ago`;
    const weeks = Math.floor(days / 7);
    if (weeks < 4) return `${weeks}w ago`;
    const months = Math.floor(days / 30);
    if (months < 12) return `${months}mo ago`;
    const years = Math.floor(days / 365);
    return `${years}y ago`;
  } catch {
    return '';
  }
}

function getListInnerWidth(container) {
  try {
    const w = typeof container.width === 'number' ? container.width : (parseInt(container.width, 10) || 40);
    // border 2 + list left/right padding approx 2
    return Math.max(10, w - 4);
  } catch { return 40; }
}

function formatListRow(file, meta, innerWidth) {
  const id = meta?.id || path.basename(file.path);
  const ageSrc = meta?.startTime ? new Date(meta.startTime) : new Date(file.mtime);
  const age = formatRelativeAge(ageSrc);
  const left = chalk.hex(THEME.orange)(id);
  const right = chalk.hex(THEME.gray)(age);
  const rawLeft = id;
  const rawRight = age;
  const pad = Math.max(1, innerWidth - rawLeft.length - rawRight.length);
  return `${left}${' '.repeat(pad)}${right}`;
}

// meta quick extraction moved to utils/metaQuick

function buildDialogPreviewBlocks(messages, { hide = [], wrapWidth } = {}) {
  const items = selectRecentDialogMessages(messages, { limit: Number.POSITIVE_INFINITY }).filter(it => {
    if (it.role === 'user' && hide.includes('user')) return false;
    if (it.role === 'assistant' && hide.includes('assistant')) return false;
    return true;
  }).map(it => ({ ...it, text: safeString(it.text).replace(/\r/g, '') }));
  return formatPreviewBlocks(items, { chalkLib: chalk, wrapWidth });
}

export async function pickSessionSplitTUI(root, presetList = null, options = {}) {
  const { hide = [], currentDirOnly = false, workspaceMode = false } = options || {};
  const files = presetList || await listSessionFiles(root);
  if (!files.length) return null;

  // Use a custom blessed program with extended terminfo disabled.
  // Some xterm-256color terminfo entries contain extended caps (e.g., Setulc)
  // that blessed cannot compile due to malformed strings. Disabling extended
  // terminfo avoids those problematic capabilities.
  let screen;
  try {
    const program = blessed.program({ terminal: process.env.TERM, extended: false, tput: true });
    screen = blessed.screen({ program, smartCSR: true, warnings: false, title: 'cxresume - Sessions / Preview', fullUnicode: true });
  } catch {
    // Fallback to default behavior if custom program setup fails
    screen = blessed.screen({ smartCSR: true, warnings: false, title: 'cxresume - Sessions / Preview', fullUnicode: true });
  }
  const gap = 1;
  let totalW = screen.width || 100;
  let leftW = Math.max(30, Math.floor(totalW * 0.35));

  const leftBox = blessed.box({
    top: 0,
    left: 0,
    width: leftW,
    height: '100%',
    border: { type: 'line' },
    borderColor: 'gray',
    label: `  ${chalk.hex(THEME.green)('Recent Sessions')}  `,
  });

  const header = blessed.box({
    parent: leftBox,
    top: 0,
    left: 1,
    height: 1,
    width: '100%-2',
    tags: false,
    content: chalk.hex(THEME.gray)('Usage: ') +
      chalk.hex(THEME.yellow)('↑/↓') + ' navigate ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('Enter') + ' resume ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('←/→') + ' pages ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('j/k') + ' scroll ' + chalk.hex(THEME.gray)('• ') +
      (workspaceMode ? chalk.hex(THEME.yellow)('s') + ' workspace new ' + chalk.hex(THEME.gray)('• ') : '') +
      chalk.hex(THEME.yellow)('n') + ' new ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('d') + ' delete ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('-') + ' edit options ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('c') + ' copy ID ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('f') + ' full ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('q') + ' quit'
  });

  const leftScroll = blessed.box({
    parent: leftBox,
    top: 2,
    left: 1,
    width: '100%-2',
    height: '100%-3',
    keys: false,
    mouse: true,
    vi: false,
    alwaysScroll: true,
    scrollable: true,
    scrollbar: { ch: ' ', track: { bg: 'gray' }, style: { bg: 'gray' } },
  });

  const rightBox = blessed.box({
    top: 0,
    left: leftW + gap,
    width: `100%-${leftW + gap}`,
    height: '100%',
    border: { type: 'line' },
    borderColor: 'gray',
    label: `  ${chalk.hex(THEME.green)('Conversation Preview')}  `,
  });

  const infoBox = blessed.box({
    parent: rightBox,
    top: 0,
    left: 1,
    width: '100%-2',
    height: 1,
    tags: false,
    content: '',
  });

  const preview = blessed.box({
    parent: rightBox,
    top: 2,
    left: 1,
    width: '100%-2',
    height: '100%-3',
    tags: false,
    wrap: false,
    keys: true,
    vi: true,
    mouse: true,
    alwaysScroll: true,
    scrollable: true,
    scrollbar: { ch: ' ', track: { bg: 'gray' }, style: { bg: 'gray' } },
    content: '',
  });

  screen.append(leftBox);
  screen.append(rightBox);

  const metaCache = new Map();
  const previewCache = new Map();
  let destroyed = false;
  let fullView = false;
  let modalActive = false; // block global key actions when a modal is open
  const ITEM_HEIGHT = 3;
  const ITEM_GAP = 1; // visual spacing between blocks
  let selectedIndex = 0;
  const itemBoxes = [];
  const msgSummaryCache = new Map(); // path -> { count, lastRole }

  function renderItemContent(f, meta) {
    const id = meta?.id || path.basename(f.path);
    const ageSrc = meta?.startTime ? new Date(meta.startTime) : new Date(f.mtime);
    const age = formatRelativeAge(ageSrc);
    const cwd = meta?.cwd || '';
    const sum = msgSummaryCache.get(f.path);
    const messages = typeof sum?.count === 'number' ? String(sum.count) : '-';
    const lastRole = sum?.lastRole || '-';
    const lastRoleColored = lastRole === 'Assistant' ? chalk.hex(THEME.green)(lastRole)
      : lastRole === 'User' ? chalk.hex(THEME.red)(lastRole)
      : chalk.hex(THEME.gray)(lastRole);

    const line1Left = chalk.hex(THEME.orange)(id);
    const line1Right = chalk.hex(THEME.gray)(age);
    // pad right based on inner width (use string-width for monospace alignment)
    const innerW = getListInnerWidth(leftBox);
    const pad1 = Math.max(1, innerW - stringWidth(id) - stringWidth(age));
    const line1 = `${line1Left}${' '.repeat(pad1)}${line1Right}`;

    // line2 path: try to color ~/ prefix purple, rest cyan
    let line2Path = '-';
    if (cwd) {
      const tilde = cwd.startsWith(process.env.HOME || '')
        ? cwd.replace(String(process.env.HOME), '~')
        : cwd;
      if (tilde.startsWith('~/')) {
        line2Path = chalk.hex(THEME.purple)('~/') + chalk.hex(THEME.cyan)(tilde.slice(2));
      } else {
        line2Path = chalk.hex(THEME.cyan)(tilde);
      }
    } else {
      line2Path = chalk.hex(THEME.gray)('-');
    }
    const line2 = line2Path;

    const sep = chalk.hex(THEME.gray)(' • ');
    const line3 = chalk.hex(THEME.gray)('Messages: ') + chalk.hex(THEME.yellow)(messages) + sep +
      chalk.hex(THEME.gray)('Last: ') + lastRoleColored;

    return `${line1}\n${line2}\n${line3}`;
  }

  function clearItemBoxes() {
    while (itemBoxes.length) {
      const it = itemBoxes.pop();
      try { it.detach(); } catch {}
    }
  }

  function buildItemBoxes() {
    clearItemBoxes();
    const innerW = getListInnerWidth(leftBox);
    let top = 0;
    for (let i = 0; i < pageItems.length; i++) {
      const f = pageItems[i];
      const meta = metaCache.get(f.path) || null;
      const box = blessed.box({
        parent: leftScroll,
        top,
        left: 0,
        width: '100%',
        height: ITEM_HEIGHT,
        tags: false,
        mouse: true,
        content: renderItemContent(f, meta),
      });
      // click to select
      box.on('click', async () => {
        setSelectedIndex(i);
        await updatePreviewForIndex(i);
        loadMeta(i);
        loadSummary(i);
      });
      itemBoxes.push(box);
      top += ITEM_HEIGHT + ITEM_GAP;
    }
    leftScroll.setContent('');
  }

  function updateItemBox(i) {
    const box = itemBoxes[i];
    if (!box) return;
    const f = pageItems[i];
    const meta = metaCache.get(f.path) || null;
    box.setContent(renderItemContent(f, meta));
  }

  function repaintListSelection() {
    for (let i = 0; i < itemBoxes.length; i++) {
      const box = itemBoxes[i];
      if (!box) continue;
      if (i === selectedIndex) {
        box.style = { bg: 'blue', fg: 'black', bold: true };
      } else {
        box.style = { bg: null, fg: 'white', bold: false };
      }
    }
  }

  function ensureSelectedVisible() {
    try {
      const y = selectedIndex * (ITEM_HEIGHT + ITEM_GAP);
      if (typeof leftScroll.scrollTo === 'function') leftScroll.scrollTo(y);
    } catch {}
  }

  function setSelectedIndex(idx) {
    if (idx < 0 || idx >= pageItems.length) return;
    selectedIndex = idx;
    repaintListSelection();
    ensureSelectedVisible();
    screen.render();
  }

  function getPreviewInnerWidth() {
    try {
      const total = typeof screen.width === 'number' ? screen.width : parseInt(screen.width, 10) || 100;
      const rbWidth = fullView ? total : (total - (leftW + gap)); // rightBox total width including borders
      const contentWidth = rbWidth - 2; // minus rightBox border
      return Math.max(10, contentWidth);
    } catch { return 80; }
  }

  function applyLayout() {
    totalW = screen.width || totalW;
    if (!fullView) {
      leftW = Math.max(30, Math.floor(totalW * 0.35));
      leftBox.left = 0;
      leftBox.width = leftW;
      leftBox.height = '100%';
      leftBox.show();
      rightBox.top = 0;
      rightBox.left = leftW + gap;
      rightBox.width = `100%-${leftW + gap}`;
      rightBox.height = '100%';
    } else {
      leftBox.hide();
      rightBox.top = 0;
      rightBox.left = 0;
      rightBox.width = '100%';
      rightBox.height = '100%';
    }
    // rebuild item content widths
    for (let i = 0; i < itemBoxes.length; i++) updateItemBox(i);
  }
  screen.on('resize', () => { applyLayout(); screen.render(); });
  let editedArgs = '';

  // Pagination state
  const ITEMS_PER_PAGE = 30;
  let currentPage = 0;
  let pageItems = [];

  let visibleFiles = files.slice();
  if (currentDirOnly) {
    // Pre-filter visible files asynchronously based on cwd
    (async () => {
      const matches = [];
      for (let i = 0; i < files.length; i++) {
        try {
          const meta = await extractSessionMetaQuick(files[i].path);
          if (meta.cwd && path.resolve(meta.cwd) === path.resolve(process.cwd())) {
            matches.push(files[i]);
          }
        } catch {}
      }
      if (!destroyed && matches.length) {
        visibleFiles = matches.sort((a,b) => b.mtime - a.mtime);
        currentPage = 0;
        updatePageItems();
        screen.render();
        try { idxLoad = 0; } catch {}
        try { idxSum = 0; } catch {}
        await updatePreviewForIndex(0);
      }
    })();
  }

  function updatePageItems() {
    const start = currentPage * ITEMS_PER_PAGE;
    const end = Math.min(visibleFiles.length, start + ITEMS_PER_PAGE);
    pageItems = visibleFiles.slice(start, end);
    buildItemBoxes();
    selectedIndex = 0;
    repaintListSelection();
  }

  function updateHeader() {
    const totalPages = Math.max(1, Math.ceil(visibleFiles.length / ITEMS_PER_PAGE));
    const info = `Page ${currentPage + 1}/${totalPages} | Showing ${pageItems.length}/${visibleFiles.length}`;
    const opts = editedArgs ? ` | Options: ${editedArgs}` : '';
    header.setContent(
      chalk.hex(THEME.gray)('Usage: ') +
      chalk.hex(THEME.yellow)('↑/↓') + ' navigate ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('Enter') + ' resume ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('←/→') + ' pages ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('j/k') + ' scroll ' + chalk.hex(THEME.gray)('• ') +
      (workspaceMode ? chalk.hex(THEME.yellow)('s') + ' workspace new ' + chalk.hex(THEME.gray)('• ') : '') +
      chalk.hex(THEME.yellow)('n') + ' new ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('d') + ' delete ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('-') + ' edit options ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('c') + ' copy ID ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('f') + ' full ' + chalk.hex(THEME.gray)('• ') +
      chalk.hex(THEME.yellow)('q') + ` quit${opts} ` + chalk.hex(THEME.gray)('• ') + info
    );
  }

  function refreshListRow(i) {
    const f = pageItems[i];
    if (!f) return;
    updateItemBox(i);
  }

  async function loadSummary(i) {
    const f = pageItems[i];
    if (!f) return;
    if (msgSummaryCache.has(f.path)) return;
    try {
      const parsed = await parseSessionFile(f.path);
      const dialog = (parsed.messages || []).filter(m => m && (m.role === 'user' || m.role === 'assistant'));
      const last = dialog.length ? dialog[dialog.length - 1] : null;
      const lastRole = last ? (last.role === 'user' ? 'User' : 'Assistant') : '-';
      msgSummaryCache.set(f.path, { count: dialog.length, lastRole });
      updateItemBox(i);
      // also update infoBox if this is current selection
      if (i === selectedIndex) {
        try { setInfoDisplay(parsed.meta, f); } catch {}
      }
    } catch {}
  }

  function setInfoDisplay(meta, file) {
    try {
      const id = (meta?.id) || path.basename(file.path);
      const cwd = meta?.cwd || '';
      const started = meta?.startTime ? dayjs(meta.startTime).format('YYYY-MM-DD HH:mm:ss') : '';
      const sep = chalk.hex(THEME.gray)(' • ');
      const content =
        chalk.hex(THEME.gray)('Session: ') + chalk.hex(THEME.orange)(id) + sep +
        chalk.hex(THEME.gray)('Path: ') + (cwd ? chalk.hex(THEME.cyan)(cwd) : chalk.hex(THEME.gray)('-')) + sep +
        chalk.hex(THEME.gray)('Started: ') + (started ? chalk.hex(THEME.yellow)(started) : chalk.hex(THEME.gray)('-'));
      infoBox.setContent(content);
    } catch {
      infoBox.setContent('');
    }
  }

  async function loadMeta(i) {
    const f = pageItems[i];
    if (metaCache.has(f.path)) return;
    try {
      const meta = await extractSessionMetaQuick(f.path);
      metaCache.set(f.path, meta);
      if (!destroyed) {
        refreshListRow(i);
        screen.render();
      }
    } catch {}
  }

  async function updatePreviewForIndex(idx) {
    if (idx < 0 || idx >= pageItems.length) return;
    const f = pageItems[idx];
    function scrollPreviewToBottom() {
      try {
        if (typeof preview.setScrollPerc === 'function') {
          preview.setScrollPerc(100);
        } else if (typeof preview.scrollTo === 'function') {
          const h = typeof preview.getScrollHeight === 'function' ? preview.getScrollHeight() : Infinity;
          preview.scrollTo(h);
        }
      } catch {}
    }
    // Update info from cached meta if available
    const metaCached = metaCache.get(f.path);
    if (metaCached) setInfoDisplay(metaCached, f);

    const wrapW = getPreviewInnerWidth();
    const cacheKey = `${f.path}::${wrapW}`;
    if (previewCache.has(cacheKey)) {
      preview.setContent(previewCache.get(cacheKey));
      scrollPreviewToBottom();
      screen.render();
      return;
    }
    preview.setContent('Loading…');
    screen.render();
    try {
      const parsed = await parseSessionFile(f.path);
      try { setInfoDisplay(parsed.meta, f); } catch {}
      const dialog = (parsed.messages || []).filter(m => m && (m.role === 'user' || m.role === 'assistant'));
      const last = dialog.length ? dialog[dialog.length - 1] : null;
      const lastRole = last ? (last.role === 'user' ? 'User' : 'Assistant') : '-';
      msgSummaryCache.set(f.path, { count: dialog.length, lastRole });
      updateItemBox(idx);
      const body = buildDialogPreviewBlocks(parsed.messages, { hide, wrapWidth: wrapW });
      previewCache.set(cacheKey, body);
      if (!destroyed) {
        preview.setContent(body);
        scrollPreviewToBottom();
        screen.render();
      }
    } catch (e) {
      if (!destroyed) {
        preview.setContent('Preview failed: ' + (e?.message || e));
        screen.render();
      }
    }
  }

  // pagination init
  updatePageItems();
  updateHeader();

  // preload meta with limited concurrency
  const concurrency = 8;
  let idxLoad = 0;
  for (let c = 0; c < concurrency; c++) {
    (async function worker() {
      while (idxLoad < pageItems.length && !destroyed) {
        const i = idxLoad++;
        await loadMeta(i);
      }
    })();
  }

  // preload message summaries (counts/last role) for visible items
  let idxSum = 0;
  const sumConcurrency = 3;
  for (let c = 0; c < sumConcurrency; c++) {
    (async function worker() {
      while (idxSum < pageItems.length && !destroyed) {
        const i = idxSum++;
        await loadSummary(i);
      }
    })();
  }

  setSelectedIndex(0);
  leftScroll.focus();
  await updatePreviewForIndex(0);

  const navKeys = ['up','down','pageup','pagedown','home','end'];
  for (const k of navKeys) {
    screen.key(k, async () => {
      if (modalActive) return;
      let sel = selectedIndex;
      switch (k) {
        case 'up': sel = Math.max(0, sel - 1); break;
        case 'down': sel = Math.min(pageItems.length - 1, sel + 1); break;
        case 'pageup': sel = Math.max(0, sel - 5); break;
        case 'pagedown': sel = Math.min(pageItems.length - 1, sel + 5); break;
        case 'home': sel = 0; break;
        case 'end': sel = pageItems.length - 1; break;
      }
      setSelectedIndex(sel);
      await updatePreviewForIndex(sel);
      loadMeta(sel);
      loadSummary(sel);
      if (sel + 1 < pageItems.length) loadMeta(sel + 1);
      if (sel - 1 >= 0) loadMeta(sel - 1);
      if (sel + 1 < pageItems.length) loadSummary(sel + 1);
      if (sel - 1 >= 0) loadSummary(sel - 1);
    });
  }

  // Page navigation
  screen.key(['left'], async () => {
    if (modalActive) return; // ignore when modal
    if (currentPage > 0) {
      currentPage--;
      updatePageItems();
      updateHeader();
      // reload meta & summary for new page
      idxLoad = 0;
      idxSum = 0;
      await updatePreviewForIndex(0);
    }
  });
  screen.key(['right'], async () => {
    if (modalActive) return; // ignore when modal
    const totalPages = Math.ceil(visibleFiles.length / ITEMS_PER_PAGE);
    if (currentPage < totalPages - 1) {
      currentPage++;
      updatePageItems();
      updateHeader();
      idxLoad = 0;
      idxSum = 0;
      await updatePreviewForIndex(0);
    }
  });

  // preview scroll via j/k
  screen.key(['j'], () => { if (modalActive) return; preview.scroll(1); screen.render(); });
  screen.key(['k'], () => { if (modalActive) return; preview.scroll(-1); screen.render(); });

  // Toggle full view
  screen.key(['f'], () => {
    if (modalActive) return;
    fullView = !fullView;
    applyLayout();
    try { if (typeof preview.setScrollPerc === 'function') preview.setScrollPerc(100); } catch {}
    screen.render();
  });

  function askEditOptions() {
    const overlay = blessed.box({
      parent: screen,
      top: 'center', left: 'center', width: '80%', height: 7,
      border: { type: 'line' }, label: ' Edit Codex Options ', borderColor: 'blue',
      keys: true
    });
    const prompt = blessed.text({ parent: overlay, top: 1, left: 2, content: chalk.hex(THEME.gray)('Enter extra command arguments (Enter to confirm / Esc to cancel):') });
    const input = blessed.textbox({ parent: overlay, top: 3, left: 2, width: '95%', height: 1, inputOnFocus: true, keys: true, mouse: true, value: editedArgs });
    input.focus();
    function cleanup() { overlay.destroy(); screen.render(); }
    input.key('escape', () => cleanup());
    input.on('submit', (val) => { editedArgs = String(val || ''); updateHeader(); cleanup(); });
    screen.render();
  }

  function selectedFile() { return pageItems[selectedIndex]; }

  // Copy session id (relative path)
  screen.key(['c'], async () => {
    const f = selectedFile();
    if (!f) return;
    let meta = metaCache.get(f.path);
    if (!meta) { try { meta = await extractSessionMetaQuick(f.path); metaCache.set(f.path, meta); } catch {} }
    const id = meta?.id || f.rel || f.path;
    try { await clipboard.write(id); } catch {}
  });

  async function confirmDeleteDialog({ id, filePath }) {
    modalActive = true;
    return await new Promise(resolve => {
      const overlay = blessed.box({
        parent: screen,
        top: 'center', left: 'center', width: '80%', height: 10,
        border: { type: 'line' }, label: ' Confirm Delete ', borderColor: 'yellow',
        keys: true
      });
      blessed.box({
        parent: overlay,
        top: 1, left: 2, width: '95%', height: 3,
        tags: false,
        content: 'Delete this session? This will remove the jsonl file.\n' +
                 `ID: ${id}\n` +
                 `File: ${filePath}`
      });
      blessed.box({
        parent: overlay,
        top: 5, left: 2, width: '95%', height: 1,
        content: chalk.hex(THEME.gray)('Use [1m←/→[0m to choose, Enter confirm, Y=Yes, N=No, Esc cancel')
      });

      // Buttons row
      const btnRow = blessed.box({ parent: overlay, top: 7, left: 0, width: '100%', height: 1 });
      const yesBtn = blessed.box({ parent: btnRow, top: 0, left: '35%', width: 8, height: 1, content: '  Yes  ' });
      const noBtn  = blessed.box({ parent: btnRow, top: 0, left: '55%', width: 8, height: 1, content: '  No   ' });
      let selected = 1; // default to No for safety
      function paint() {
        if (selected === 0) {
          yesBtn.style = { bg: 'green', fg: 'black', bold: true };
          noBtn.style = { bg: null, fg: 'white', bold: false };
        } else {
          yesBtn.style = { bg: null, fg: 'white', bold: false };
          noBtn.style  = { bg: 'red', fg: 'black', bold: true };
        }
      }
      paint();

      function finish(ans) {
        try { overlay.destroy(); } catch {}
        modalActive = false;
        screen.render();
        resolve(ans);
      }

      overlay.key(['left'], () => { selected = 0; paint(); screen.render(); });
      overlay.key(['right'], () => { selected = 1; paint(); screen.render(); });
      overlay.key(['y','Y'], () => finish(true));
      overlay.key(['n','N'], () => finish(false));
      overlay.key(['enter'], () => finish(selected === 0));
      overlay.key(['escape'], () => finish(false));
      overlay.focus();
      screen.render();
    });
  }

  async function deleteSelectedFile() {
    const f = selectedFile();
    if (!f) return;
    let meta = metaCache.get(f.path);
    if (!meta) { try { meta = await extractSessionMetaQuick(f.path); metaCache.set(f.path, meta); } catch {} }
    const id = meta?.id || f.rel || f.path;

    const confirmed = await confirmDeleteDialog({ id, filePath: f.path });
    if (!confirmed) return;

    try {
      await fs.unlink(f.path);
    } catch (e) {
      // show error dialog
      await new Promise(res => {
        const overlay = blessed.box({ parent: screen, top: 'center', left: 'center', width: '70%', height: 7, border: { type: 'line' }, label: ' Delete Failed ', keys: true });
        blessed.box({ parent: overlay, top: 1, left: 2, width: '95%', height: 3, content: String(e?.message || e) });
        overlay.key(['enter','escape','q'], () => { try { overlay.destroy(); } catch {} screen.render(); res(); });
        overlay.focus();
        screen.render();
      });
      return;
    }

    // Remove from caches
    try { metaCache.delete(f.path); } catch {}
    try { msgSummaryCache.delete(f.path); } catch {}
    try {
      for (const key of Array.from(previewCache.keys())) {
        if (key.startsWith(`${f.path}::`)) previewCache.delete(key);
      }
    } catch {}

    // Remove from lists
    visibleFiles = visibleFiles.filter(it => it.path !== f.path);

    // If no more files, exit
    if (!visibleFiles.length) {
      destroyed = true;
      screen.destroy();
      return await Promise.resolve();
    }

    // Adjust current page if needed
    const totalPages = Math.max(1, Math.ceil(visibleFiles.length / ITEMS_PER_PAGE));
    if (currentPage >= totalPages) currentPage = totalPages - 1;

    const prevSel = selectedIndex;
    updatePageItems();
    updateHeader();
    // pick nearest index
    const newSel = Math.max(0, Math.min(prevSel, pageItems.length - 1));
    setSelectedIndex(newSel);
    await updatePreviewForIndex(newSel);
    screen.render();
  }

  applyLayout();
  return await new Promise(resolve => {
    screen.key('enter', () => {
      if (modalActive) return; // avoid conflict when a modal is open
      destroyed = true;
      const f = selectedFile();
      screen.destroy();
      resolve({ path: f.path, extraArgs: editedArgs, action: 'resume' });
    });
    // Delete selected session
    screen.key(['d'], async () => {
      if (modalActive) return;
      await deleteSelectedFile();
      // If after deletion there are no files, exit gracefully
      if (!visibleFiles.length) {
        if (!destroyed) { destroyed = true; screen.destroy(); }
        resolve(null);
      }
    });
    screen.key(['-'], () => { if (modalActive) return; askEditOptions(); });
    screen.key(['n'], async () => {
      if (modalActive) return; // prevent conflict with delete dialog using 'n'
      const f = selectedFile(); if (!f) return;
      let cwd;
      try { cwd = (await extractSessionMetaQuick(f.path))?.cwd; } catch {}
      destroyed = true;
      screen.destroy();
      resolve({ action: 'startNew', workingDir: cwd, extraArgs: editedArgs });
    });
    screen.key(['s'], () => {
      if (modalActive || !workspaceMode) return;
      destroyed = true;
      screen.destroy();
      resolve({ action: 'workspaceCreate' });
    });
    screen.key(['q','C-c','escape'], () => {
      if (modalActive) return; // do not allow quitting while a modal is open
      destroyed = true;
      screen.destroy();
      resolve(null);
    });
    screen.render();
  });
}
