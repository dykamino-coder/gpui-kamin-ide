// Mouse-tracking DECSET filter for the CLI terminal mirror.
//
// Claude CLI enables xterm mouse reporting (DECSET 1000/1002/1003/1006…).
// Once a terminal is in mouse-reporting mode, xterm.js forwards mouse drags
// to the PTY instead of selecting — plain click-drag selects NOTHING in the
// Bridge Console (classic "need Shift+drag" terminal behaviour, which nobody
// discovers). The Console is a read-mostly mirror where selection/copy matters
// far more than in-TUI mouse clicks, so we strip the mouse-mode set/reset
// sequences from the output stream before xterm ever sees them: xterm stays
// out of mouse-reporting, native drag-select works, and the wheel scrolls the
// xterm buffer. The CLI simply behaves as if the terminal had no mouse
// support (Claude Code's TUI is keyboard-first — nothing breaks).

// DEC private modes that switch on mouse reporting / encodings.
const MOUSE_MODES = new Set(['9', '1000', '1001', '1002', '1003', '1005', '1006', '1015', '1016'])

const DECSET_RE = /\x1b\[\?([0-9;]+)([hl])/g

/** Remove mouse-tracking modes from DECSET/DECRST sequences. Non-mouse modes
 *  in the same sequence are preserved (`CSI ? 25;1000 h` → `CSI ? 25 h`). */
export function stripMouseTracking(chunk: string): string {
  if (!chunk.includes('\x1b[?')) return chunk
  return chunk.replace(DECSET_RE, (full, params: string, hl: string) => {
    const parts = params.split(';')
    const kept = parts.filter(p => !MOUSE_MODES.has(p))
    if (kept.length === parts.length) return full
    if (kept.length === 0) return ''
    return `\x1b[?${kept.join(';')}${hl}`
  })
}

// A DECSET split across two PTY chunks would slip past the regex — hold back
// any trailing incomplete escape prefix and prepend it to the next chunk.
// Matches: ESC · ESC[ · ESC[? · ESC[?digits/semicolons (still unterminated).
const TRAILING_PARTIAL_RE = /\x1b(?:\[(?:\?[0-9;]*)?)?$/

/** Split a chunk into [writable, carry]: `carry` is a trailing partial escape
 *  sequence to hold until the next chunk arrives. */
export function splitTrailingEscape(chunk: string): [string, string] {
  const m = TRAILING_PARTIAL_RE.exec(chunk)
  if (!m || m.index === chunk.length) return [chunk, '']
  return [chunk.slice(0, m.index), chunk.slice(m.index)]
}
