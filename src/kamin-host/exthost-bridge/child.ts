// Ext-host CHILD entry — `node kamin-host.mjs --role=exthost`. Runs ONLY the
// extension host; a native crash here takes down just this process and the
// parent respawns it. Reaches the parent's services (fs, index, sessions,
// renderer link) over the fork-IPC RpcEndpoint; runs config/storage/documents/
// editors in-process (they only back extensions) seeded from the parent.
import type { KaminSession, SessionsSnapshot } from "../../api/types.js"
import type { WatchEvent } from "../../exthost/host-services.js"
import { startExtHost } from "../../exthost/index.js"
import { reviveCommandArg } from "../mcp-uri.js"
import { SHELL_OPEN_EXTERNAL, SHELL_READ_CLIPBOARD, SHELL_SHOW_INPUT_BOX, SHELL_SHOW_MESSAGE, SHELL_SHOW_OPEN_DIALOG, SHELL_SHOW_SAVE_DIALOG, SHELL_SHOW_QUICK_PICK } from "../protocol.js"
import type { RpcEndpoint } from "../rpc.js"
import * as config from "../services/config.js"
import * as storage from "../services/storage.js"
import { setWorkspaceFolderMirror } from "../services/workspace.js"
import { installChildCrashContainment } from "./child-crash.js"
import { buildInvokeTable } from "./child-invoke.js"
import { buildEnvHost, buildSessionsHost, buildStorageHost, buildWorkspaceHost, type ChildDeps } from "./child-proxies.js"
import {
  CHILD_INVOKE, CHILD_READY,
  type ExtHostSeed,
  HOST_BROADCAST, HOST_REQUEST_RENDERER, HOST_SEED,
  MIRROR_SESSIONS, MIRROR_SESSIONS_ACTIVE, MIRROR_WATCH, MIRROR_WORKSPACE,
} from "./protocol.js"

export async function runExtHostChild(endpoint: RpcEndpoint): Promise<void> {
  const call = <T,>(method: string, ...params: unknown[]): Promise<T> => endpoint.call<T>(method, ...params)
  const requestRenderer = <T,>(method: string, ...params: unknown[]): Promise<T> =>
    call<T>(HOST_REQUEST_RENDERER, method, params)
  const broadcast = (channel: string, payload: unknown): void => { endpoint.emit(HOST_BROADCAST, { channel, payload }) }

  const markBooted = installChildCrashContainment(broadcast)

  const seed = await call<ExtHostSeed>(HOST_SEED)
  setWorkspaceFolderMirror(seed.workspaceFolder)
  config.initConfig(seed.dataDir)
  storage.initStorage(seed.dataDir)

  // ── Mirror state fed by the parent's MIRROR_* pushes ──
  const watchSinks = new Set<(events: readonly WatchEvent[]) => void>()
  let sessionsSnap = seed.sessions.snapshot
  let sessionsActive = seed.sessions.active
  const sessionChange = new Set<(s: SessionsSnapshot) => void>()
  const sessionActiveChange = new Set<(s: KaminSession | null) => void>()
  endpoint.onEvent((channel, payload) => {
    if (channel === MIRROR_WORKSPACE) setWorkspaceFolderMirror(payload as string | null)
    else if (channel === MIRROR_WATCH) { for (const fn of watchSinks) fn(payload as WatchEvent[]) }
    else if (channel === MIRROR_SESSIONS) { sessionsSnap = payload as SessionsSnapshot; for (const fn of sessionChange) fn(sessionsSnap) }
    else if (channel === MIRROR_SESSIONS_ACTIVE) { sessionsActive = payload as KaminSession | null; for (const fn of sessionActiveChange) fn(sessionsActive) }
  })

  const deps: ChildDeps = {
    call,
    requestRenderer,
    watchFiles: (fn) => { watchSinks.add(fn); return () => watchSinks.delete(fn) },
    sessions: {
      snapshot: () => sessionsSnap,
      active: () => sessionsActive,
      onChange: (fn) => { sessionChange.add(fn); return { dispose: () => { sessionChange.delete(fn) } } },
      onActiveChange: (fn) => { sessionActiveChange.add(fn); return { dispose: () => { sessionActiveChange.delete(fn) } } },
    },
  }

  // Диагностика бюджета старта: сколько занимает подъём exthost до момента,
  // когда становится доступна таблица CHILD_INVOKE (до этого ВСЕ RPC ждут).
  // Монитор блокировок цикла событий: тик каждые 50 мс, лог при задержке
  // больше 400 мс. RPC у ребёнка отвечали через 10-25 с при том, что таблица
  // жива за 8 мс — значит кто-то ДЕРЖИТ поток синхронной работой.
  {
    let last = Date.now()
    const LAG_MS = 400
    const TICK_MS = 50
    const t = setInterval(() => {
      const now = Date.now()
      const lag = now - last - TICK_MS
      if (lag > LAG_MS) console.error(`[loop блок] ${lag}ms`)
      last = now
    }, TICK_MS)
    t.unref()
  }
  const tStart = Date.now()
  const host = await startExtHost({
    onHostReady: markBooted, // contain extension async errors during activation
    builtinDir: seed.builtinDir,
    userExtDir: seed.userExtDir,
    broadcast,
    showMessage: (severity, message, items) => requestRenderer(SHELL_SHOW_MESSAGE, severity, message, items),
    showInputBox: (opts) => requestRenderer(SHELL_SHOW_INPUT_BOX, opts),
    showQuickPick: (items, options) => requestRenderer(SHELL_SHOW_QUICK_PICK, items, options),
    showOpenDialog: (options) => requestRenderer(SHELL_SHOW_OPEN_DIALOG, options),
    showSaveDialog: (options) => requestRenderer(SHELL_SHOW_SAVE_DIALOG, options),
    openExternal: (target) => requestRenderer(SHELL_OPEN_EXTERNAL, target),
    readClipboard: () => requestRenderer(SHELL_READ_CLIPBOARD),
    emitOutputEvent: (event) => { broadcast("kamin:output:event", event) },
    workspaceHost: buildWorkspaceHost(deps),
    storage: buildStorageHost(requestRenderer),
    env: buildEnvHost(deps),
    webviewStore: { load: () => storage.loadWebviewPanels(), save: (panels) => { storage.saveWebviewPanels(panels) } },
    sessionsHost: buildSessionsHost(deps),
  })

  // Renderer-facing calls the parent routes here: ExtHost methods + Monaco
  // doc/editor sync. reviveCommandArg turns marshalled Uris into real ones.
  console.error(`[boot] startExtHost ${Date.now() - tStart}ms`)
  const table = buildInvokeTable(host, reviveCommandArg)
  endpoint.handle(CHILD_INVOKE, (method, ...params) => {
    const fn = table[method as string]
    if (!fn) throw new Error(`exthost child: unknown invoke "${String(method)}"`)
    return fn(...params)
  })

  console.error(`[boot] invoke-table live at ${Date.now() - tStart}ms`)
  endpoint.emit(CHILD_READY, { extensions: host.listExtensions().length })
  markBooted()

  // The renderer hydrates registry/extensions/diagnostics the instant the WS
  // connects (host READY is immediate), but that races `loadAll` above — the
  // CHILD_INVOKE handler is only registered once we reach here, so those early
  // pulls reject with "unknown invoke" and the extensions list (which, unlike
  // registry/diagnostics, has no per-item broadcast to self-heal) stays empty.
  // Now that the handler is live, tell the renderer to re-pull the list.
  // Шлём СРАЗУ САМ СПИСОК, а не приглашение перезапросить: ответный
  // `extensions:list` шёл 12-13 с (очередь за блокировками родителя), и клиент
  // узнавал об активации через 35 с при том, что она заняла меньше секунды.
  broadcast("kamin:extensions:changed", { initial: true, list: host.listExtensions() })
}
