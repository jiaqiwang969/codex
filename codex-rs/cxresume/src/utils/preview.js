import dayjs from 'dayjs';
import stringWidth from 'string-width';

// Terminal-like theme colors inspired by codex_recovery_1.html
const THEME = {
  gray: '#808080',
  green: '#5af78e',
  orange: '#ff9500',
};

export function selectRecentDialogMessages(messages, { limit = 20 } = {}) {
  if (!Array.isArray(messages) || messages.length === 0) return [];
  const subset = [];
  for (let i = messages.length - 1; i >= 0 && subset.length < limit; i--) {
    const m = messages[i];
    if (!m) continue;
    const isDialog = m.role === 'user' || m.role === 'assistant';
    if (!isDialog) continue;
    subset.push({
      role: m.role,
      text: String(m.text || ''),
      timestamp: m.timestamp ? new Date(m.timestamp) : undefined,
    });
  }
  subset.reverse();
  return subset;
}

export function formatPreviewLines(items, { color = false, chalkLib = null } = {}) {
  const chalk = color && chalkLib ? chalkLib : null;
  return items.map(it => {
    const ts = it.timestamp ? dayjs(it.timestamp).format('HH:mm:ss') : '';
    const role = it.role === 'user' ? 'User' : 'AI';
    const timeStr = chalk ? chalk.hex(THEME.gray)(`[${ts}]`) : `[${ts}]`;
    const roleStr = chalk
      ? (it.role === 'user' ? chalk.hex(THEME.orange)(role) : chalk.hex(THEME.green)(role))
      : role;
    return `${timeStr} ${roleStr}: ${it.text}`;
  });
}

// Block-style preview: each message as two parts (header + body),
// with a colored vertical bar for the role on both lines.
export function formatPreviewBlocks(items, { chalkLib = null, wrapWidth } = {}) {
  const chalk = chalkLib || null;
  const out = [];

  function wrapPlainLine(line, maxWidth) {
    const result = [];
    let cur = '';
    let width = 0;
    for (const ch of line) {
      const w = stringWidth(ch);
      if (width + w > maxWidth && cur) {
        result.push(cur);
        cur = ch;
        width = w;
      } else {
        cur += ch;
        width += w;
      }
    }
    if (cur || line === '') result.push(cur);
    return result;
  }

  for (const it of items) {
    const ts = it.timestamp ? dayjs(it.timestamp).format('HH:mm:ss') : '';
    const isUser = it.role === 'user';
    const roleLabel = isUser ? 'User' : 'AI';
    const colorHex = isUser ? THEME.orange : THEME.green;
    const bar = chalk ? chalk.hex(colorHex)('┃') : '┃';
    const barPrefix = `${bar} `;
    const prefixWidth = stringWidth('┃ ');
    const roleStr = chalk ? (isUser ? chalk.hex(THEME.orange)(roleLabel) : chalk.hex(THEME.green)(roleLabel)) : roleLabel;
    const timeStr = chalk ? chalk.hex(THEME.gray)(ts) : ts;

    // Header line: one line (rarely wraps), show role + time
    out.push(`${barPrefix}${roleStr} ${timeStr}`);

    // Body lines: split on newlines and hard-wrap so each visual line has the bar
    const body = String(it.text || '');
    const rawLines = body.split(/\n/);
    const contentWidth = (typeof wrapWidth === 'number' && wrapWidth > prefixWidth + 1)
      ? (wrapWidth - prefixWidth)
      : undefined;
    for (const raw of rawLines) {
      if (!contentWidth) {
        out.push(`${barPrefix}${raw}`);
        continue;
      }
      const segments = wrapPlainLine(raw, contentWidth);
      for (const seg of segments) {
        out.push(`${barPrefix}${seg}`);
      }
    }
    // spacing between messages
    out.push('');
  }

  return out.join('\n');
}
