// Per-server connection log ring buffer + shared types.
//
// Each MCP server keeps a bounded list of connect / test events so the
// "Test" pane can show the user what happened instead of leaving them
// staring at a 30-second timeout. The buffer survives reconnects.

import type { BrowserWindow } from '@kaminide/host-compat'

/** A single line in the per-server connection log. Shown in the "Test" pane
 *  so the user can see what happened during connect instead of just staring
 *  at a 30-second timeout. */
export interface McpTestLogEntry {
  /** Millis since epoch. Rendered as HH:MM:SS.mmm relative in the UI. */
  ts: number
  level: 'info' | 'warn' | 'error' | 'stderr' | 'stdout'
  msg: string
}

export const TEST_LOG_CAP = 300

/** Minimal shape needed for log broadcast — a server state with a testLog
 *  field. Kept structural so manager.ts can pass its `McpServerState`
 *  without a cyclic import. */
export interface TestLogBearer {
  testLog: McpTestLogEntry[]
}

/** Append a line to the ring buffer AND broadcast it to the renderer.
 *  The UI's TestLogPane subscribes to `mcp:test-log` for live updates;
 *  if it mounts mid-connect, `getTestLog` replays the buffer. */
export function appendLog(
  window: BrowserWindow,
  state: TestLogBearer | undefined,
  id: string,
  level: McpTestLogEntry['level'],
  msg: string,
): void {
  if (!state) return
  const entry: McpTestLogEntry = { ts: Date.now(), level, msg }
  state.testLog.push(entry)
  if (state.testLog.length > TEST_LOG_CAP) {
    // Drop from the head — `.shift()` in a loop would be O(n²) for a big
    // truncation, but we only ever push one at a time so one shift is fine.
    state.testLog.shift()
  }
  if (!window.isDestroyed()) {
    window.webContents.send('mcp:test-log', { id, entry })
  }
}

/** Read the full ring buffer for a server. */
export function getTestLog(state: TestLogBearer | undefined): McpTestLogEntry[] {
  return state ? state.testLog.slice() : []
}

/** Wipe the ring buffer for a server. */
export function clearTestLog(
  window: BrowserWindow,
  state: TestLogBearer | undefined,
  id: string,
): void {
  if (!state) return
  state.testLog = []
  if (!window.isDestroyed()) {
    window.webContents.send('mcp:test-log-cleared', { id })
  }
}
