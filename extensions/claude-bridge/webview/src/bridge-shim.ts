// Kamin bridge API for the webview. `window.kaminBridge` is canonical;
// `window.electronBridge` remains a deprecated compatibility alias for the
// vendored renderer. Every method maps to
// a postMessage transport call (inv/snd/sub) routed to the extension host's
// BridgeHost, which turns them back into the vendored ipcMain handlers. Runs
// before main.tsx (script order in index.html), so the copied renderer sees a
// fully-populated bridge at import time.
import { inv, snd, sub } from "./lib/bridge-transport.js"
import { INVOKE, SEND, EVENT } from "./lib/bridge-channels.js"
import { reconnectNonce } from "./signals/connection.js"

type AnyFn = (...args: unknown[]) => unknown
const bridge: Record<string, AnyFn> = {}

for (const [name, channel] of Object.entries(INVOKE)) bridge[name] = (...args) => inv(channel, ...args)
for (const [name, channel] of Object.entries(SEND)) bridge[name] = (...args) => snd(channel, ...args)
for (const [name, channel] of Object.entries(EVENT)) {
  bridge[name] = (cb) => sub(channel, cb as (...a: unknown[]) => void)
}

// ─── Specials (arg reshape / dynamic channel / sync return) ──────────────────

// saveTextAs(text, defaultName) — preload wraps both into one object arg.
bridge.saveTextAs = (text, defaultName) => inv("save-text-as", { text, defaultName })
// hooksGetLog(limit?) — preload defaults the limit to 200.
bridge.hooksGetLog = (limit) => inv("hooks:get-log", (limit as number) ?? 200)
// FilePanel PTY — preload packs args into a single object payload.
bridge.ptyWrite = (ptyId, data) => snd("pty:write", { ptyId, data })
bridge.ptyResize = (ptyId, cols, rows) => snd("pty:resize", { ptyId, cols, rows })
bridge.ptyKill = (ptyId) => snd("pty:kill", { ptyId })
// External-MCP elicitation reply uses a per-request dynamic channel.
bridge.respondMcpElicitation = (requestId, payload) => snd(`mcp-elicitation-response:${requestId as string}`, payload)
// getDroppedFilePath (Electron webUtils) удалён: не-image drop/paste ходят
// байтами через save-dropped-file (аудит #70 A3).

// getVersion() is synchronous in preload (sendSync). Prefetch once and serve
// from cache; the version string is only shown in the About/header.
let cachedVersion = ""
void inv("get-version").then((v) => { cachedVersion = typeof v === "string" ? v : "" })
bridge.getVersion = () => cachedVersion
// KaminIDE owns the window chrome — the webview has no titlebar controls.
bridge.windowIsMaximized = () => false

;(window as unknown as { kaminBridge: unknown }).kaminBridge = bridge
;(window as unknown as { electronBridge: unknown }).electronBridge = bridge

// Reconnect recovery. When the extension's WS to the server drops and reconnects
// mid-session, the component-level event subs (registered by useBridgeListeners
// in a useEffect) could end up stale — postMessages keep arriving but streaming +
// JSONL stop being applied (root cause in fix_webview_subs_drop_on_reconnect).
// The OLD fix was a full `location.reload()`, which killed the whole iframe (JS
// state + jsonlEntriesByTab wiped, 2 MB bundle re-parsed, scroll lost) on EVERY
// reconnect. Instead, bump a module-level signal: useBridgeListeners depends on it
// and cleanly re-subscribes (its effect cleanup removes every old handler first,
// so no double-registration) and re-pulls missed entries via a replay — all
// without reloading the iframe. Registered at MODULE load so it survives the wedge.
sub("bridge:reconnected", () => { reconnectNonce.value++ })
