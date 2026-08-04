import { signal } from '@preact/signals'

// Bumped on every mid-session WS reconnect (from bridge-shim's `bridge:reconnected`
// handler). useBridgeListeners depends on it so it cleanly re-subscribes its event
// handlers on reconnect — replacing the old `location.reload()` sledgehammer that
// wiped the whole iframe (JS state + jsonlEntriesByTab) and re-parsed the 2 MB
// bundle. The chat iframe now survives a reconnect; only missed entries are
// re-pulled via a replay.
export const reconnectNonce = signal(0)

export const mcpLoading = signal(true)
export const tabPromptReady = signal<Map<string, boolean>>(new Map())
/** Per-tab timestamp of last input send — prevents prompt echo from re-enabling input */
export const lastSendAt = signal<Map<string, number>>(new Map())
/**
 * Per-tab flag: is the `>` (or `›`) prompt line currently visible in the xterm
 * viewport? Updated from `terminal.onRender` in useTerminal. Used to AND-gate
 * `tabPromptReady` — prevents the send-arrow returning before the CLI has
 * actually redrawn its prompt line. Absent key (undefined) means the terminal
 * hasn't mounted yet — callers must fall back to `true` so we don't block.
 */
export const tabPromptVisible = signal<Map<string, boolean>>(new Map())
