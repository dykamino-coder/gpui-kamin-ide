import type { Terminal } from '@xterm/xterm'
import type { FitAddon } from '@xterm/addon-fit'
import { storage } from '@bridge/storage'

export interface TerminalEntry {
  terminal: Terminal
  fitAddon: FitAddon
  needsClear: boolean
  /** Pending output chunks waiting to be flushed in a single xterm.write(). */
  pendingOutput: string[]
  /** Active rAF handle (if any) for the scheduled flush. */
  rafHandle: number | null
  /** Trailing partial escape sequence held back by the mouse-tracking filter
   *  (a DECSET split across flushes) — prepended to the next flush. */
  outputCarry: string
}

/** Global registry: tabId → terminal entry. Used by bridge.onOutput routing in App.tsx */
export const terminalRegistry = new Map<string, TerminalEntry>()

/** localStorage key for a tab's xterm scrollback snapshot (restored on mount so
 *  the console survives renderer reloads). Single source of truth — useTerminal
 *  writes it, resetConsoleForReconnect drops it. */
export const terminalSnapshotKey = (tabId: string): string => `xterm-snapshot:${tabId}`

/** Prepare a tab's console to be REPLACED by a freshly-resumed PTY (the manual
 *  Reconnect button ends the server session and `--resume`s a brand-new CLI
 *  process — the same operation as a model/effort restart, which clears via
 *  `needsClear`). Without this the resumed session's redraw layered on top of the
 *  dead session's screen, so the console showed stale content that "never
 *  changed" on reconnect. We DEFER the wipe to the resumed PTY's first output
 *  (so the old screen stays visible until the new one actually streams, matching
 *  onSessionRestarted) AND drop the persisted snapshot so a remount in the
 *  reconnect window can't restore the dead session's buffer. */
export function resetConsoleForReconnect(tabId: string): void {
  const entry = terminalRegistry.get(tabId)
  if (entry) {
    entry.needsClear = true
    entry.pendingOutput.length = 0
    entry.outputCarry = ''
  }
  try { storage.removeItem(terminalSnapshotKey(tabId)) } catch { /* storage disabled/quota */ }
}
