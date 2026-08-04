// PARENT boot — the kamin-host process that owns the user-facing services (PTY,
// fs, file index, watcher, workspace, sessions) and the renderer WS link, and
// forks the ext-host CHILD for native-crash isolation. Split from kamin-host.ts
// (now a thin role dispatcher) so the child path never statically pulls this.
import { mkdirSync } from "node:fs"
import { join } from "node:path"
import type { HostFs } from "../exthost/host-services.js"
import { exthostMethods } from "./exthost-bridge/parent-methods.js"
import { forkExtHost } from "./exthost-bridge/parent.js"
import { createFreezeWatchdog } from "./freeze-watchdog.js"
import {
  EVT_FATAL, EVT_READY,
  HOST_DISPOSE, HOST_EXECUTE_COMMAND, HOST_LIST_EXTENSIONS, HOST_SNAPSHOT,
} from "./protocol.js"
import type { RpcEndpoint } from "./rpc.js"
import * as fileIndex from "./services/file-index.js"
import * as fsIo from "./services/file-io.js"
import { buildServiceMethods, disposeServices } from "./services/index.js"
import * as sessions from "./services/sessions.js"
import * as watcher from "./services/watcher.js"
import * as workspace from "./services/workspace.js"
import { startWsRpcServer, type WsRpcServer } from "./ws-server.js"

// Отключение простаивающей сессии в dev-обвязке (было магическое 90_000).
const IDLE_DISCONNECT_MS = 90_000

// Must exceed GRACEFUL_EXIT_DELAY_MS (250ms) in services: exiting sooner kills
// the PTY teardown timer before it fires and orphans the shells it owns.
const EXIT_GRACE_MS = 500

function argValue(name: string): string | null {
  const prefix = `--${name}=`
  const hit = process.argv.find((a) => a.startsWith(prefix))
  return hit ? hit.slice(prefix.length) : null
}

/** Raw filesystem surface the child reaches over RPC — mirrors the in-process
 *  `workspace.fs` wiring. `delete` honors the recursive flag (vscode default
 *  false → non-empty dir throws). */
function buildHostFs(): HostFs {
  return {
    stat: (p) => fsIo.statPath(p),
    readFile: (p) => fsIo.readBytes(p),
    writeFile: (p, data) => fsIo.writeBytes(p, data),
    readDirectory: async (p) => (await fsIo.listDir(p)).map((e) => [e.name, e.type] as [string, typeof e.type]),
    createDirectory: (p) => fsIo.makeDir(p),
    // useTrash — семантика vscode workspace.fs.delete: корзина вместо
    // перманентного удаления (опция раньше терялась мостом — ревью).
    delete: (p, recursive, useTrash) => (useTrash ? fsIo.trashPath(p) : fsIo.deletePath(p, recursive)),
    rename: (src, dst, overwrite) => fsIo.movePath(src, dst, overwrite),
    copy: (src, dst, overwrite) => fsIo.copyPath(src, dst, overwrite),
  }
}

export async function runHost(endpoint: RpcEndpoint): Promise<void> {
  // KAMIN_CPU_PROF_LIVE=1 — диагностика стартовых блокировок event loop
  // РОДИТЕЛЯ (наблюдались куски по 4-7с, душившие все ответы вебвью): пишет
  // CPU-профиль первых 90с в temp, не дожидаясь graceful-выхода (--cpu-prof
  // при жёстком завершении не флашится, а --inspect через env наследуется
  // exthost-ребёнком и конфликтует портом).
  if (process.env.KAMIN_CPU_PROF_LIVE === "1") {
    void (async () => {
      const inspector = await import("node:inspector")
      const { writeFileSync } = await import("node:fs")
      const os = await import("node:os")
      const session = new inspector.Session()
      session.connect()
      session.post("Profiler.enable", () => {
        session.post("Profiler.start", () => {
          setTimeout(() => {
            session.post("Profiler.stop", (err, res) => {
              if (!err) {
                const out = join(os.tmpdir(), `kamin-host-${process.pid}.cpuprofile`)
                writeFileSync(out, JSON.stringify(res.profile))
                console.error(`[cpu-prof] written ${out}`)
              }
              session.disconnect()
            })
          }, IDLE_DISCONNECT_MS)
        })
      })
    })()
  }
  // The parent runs only services now — an uncaught throw here is a genuine host
  // bug (extensions are isolated in the child). Escalate so the shell restarts us.
  const onFatal = (err: unknown, kind: string): void => {
    console.error(`kamin-host: ${kind}`, err)
    endpoint.emit(EVT_FATAL, { message: err instanceof Error ? err.message : String(err) })
    process.exit(1)
  }
  process.on("uncaughtException", (err) => { onFatal(err, "uncaughtException") })
  process.on("unhandledRejection", (reason) => { onFatal(reason, "unhandledRejection") })

  const builtinDir = argValue("builtin-dir")
  if (!builtinDir) throw new Error("kamin-host: missing --builtin-dir argument")
  const dataDir = argValue("data-dir")
  if (!dataDir) throw new Error("kamin-host: missing --data-dir argument")
  // Caches (file index) live in LOCAL app data — they are rebuildable and big,
  // so they must not ride the roaming profile. Older shells don't pass it.
  const cacheDir = argValue("cache-dir") ?? dataDir
  // Extensions run inside THIS process, so the env is how they learn where the
  // local (non-roaming) cache lives. The Bridge extension mirrors transcripts
  // there — tens of MB per session, which must not ride the roaming profile,
  // and its own globalStorage is roaming.
  process.env.KAMIN_CACHE_DIR = cacheDir
  const userExtDir = join(dataDir, "extensions")
  mkdirSync(userExtDir, { recursive: true })

  // Freeze watchdog: sees the renderer's heartbeat stop and logs the culprit to
  // <dataDir>/freeze.log. `noteSend` records what we push at the renderer.
  const freezeWatchdog = createFreezeWatchdog(dataDir)

  let ws: WsRpcServer | null = null
  const broadcast = (channel: string, payload: unknown): void => {
    freezeWatchdog.noteSend(channel)
    ws?.broadcast(channel, payload)
  }
  const request = <T,>(method: string, ...params: unknown[]): Promise<T> =>
    ws ? ws.request<T>(method, ...params) : Promise.reject(new Error("kamin-host: renderer not ready"))

  const methods = buildServiceMethods({
    dataDir,
    cacheDir,
    legacyWorkspacePath: argValue("legacy-workspace"),
    openFolderPath: argValue("open-folder"),
    broadcast,
    ownExtHostServices: false, // extensions run in the forked child
  })

  const exthost = forkExtHost({
    dataDir, builtinDir, userExtDir, broadcast, request,
    fs: buildHostFs(),
    // Await the index walk before returning: the child's `workspaceContains`
    // activation pass calls this at boot, and a half-built (or empty, mid-walk)
    // list would make extensions like rust-analyzer (workspaceContains:Cargo.toml)
    // silently never activate. ensureIndex is idempotent — instant once built.
    listFiles: async () => {
      const root = workspace.getWorkspaceFolder().path
      if (root) await fileIndex.ensureIndex(root)
      return fileIndex.getIndex().map((f) => ({ rel: f.rel, abs: f.abs }))
    },
    workspace: {
      getFolderPath: () => workspace.getWorkspaceFolder().path,
      onChange: (fn) => workspace.onWorkspaceChange(fn),
    },
    sessions: {
      list: () => sessions.listSessions(),
      getActive: () => sessions.getActiveSession(),
      onChange: (fn) => { sessions.onSessionsChange(fn) },
      onActiveChange: (fn) => { sessions.onActiveSessionChange(fn) },
      create: (projectId, name) => sessions.newSession(projectId, name),
      setActive: (id) => { sessions.setActiveSession(id) },
      update: (id, patch) => sessions.updateSession(id, patch),
      delete: (id) => { sessions.deleteSession(id) },
    },
    watch: { subscribe: (fn) => { watcher.addEventSink(fn) } },
  })

  for (const [m, fn] of exthostMethods(exthost, userExtDir)) methods.set(m, fn)

  // Renderer liveness beacon → freeze watchdog (see freeze-watchdog.ts).
  methods.set("kamin:diag:hb", (hb) => { freezeWatchdog.onHeartbeat(hb); return undefined })

  // Диагностика памяти (#74 RAM-диета): срез usage и heap-снапшоты ОБОИХ
  // node-процессов по запросу. Внешний инспектор (`process._debugProcess`)
  // блокируется корпоративным DLP (OpenProcess errno 87) — единственный
  // надёжный путь к ретейнерам прогретого процесса — изнутри.
  methods.set("kamin:diag:memory", async () => ({
    host: {
      pid: process.pid,
      ...process.memoryUsage(),
      // Главный подозреваемый в heap хоста — файловый индекс воркспейса.
      filesIndexed: fileIndex.getIndex().length,
    },
    exthost: await exthost.invoke("diag:memory").catch(() => null),
  }))
  methods.set("kamin:diag:heapSnapshot", async () => {
    const dir = join(cacheDir, "heap")
    mkdirSync(dir, { recursive: true })
    const v8 = await import("node:v8")
    return {
      host: v8.writeHeapSnapshot(join(dir, `host-${process.pid}.heapsnapshot`)),
      exthost: await exthost.invoke("diag:heapSnapshot", dir).catch(() => null),
    }
  })

  ws = await startWsRpcServer(methods)

  // Shell aliases + lifecycle on the parentPort (Rust) link.
  for (const [m, fn] of methods) endpoint.handle(m, fn)
  endpoint.handle(HOST_SNAPSHOT, () => exthost.invoke("snapshot"))
  endpoint.handle(HOST_EXECUTE_COMMAND, (id, ...args) => exthost.invoke("executeCommand", id, ...args))
  endpoint.handle(HOST_LIST_EXTENSIONS, () => exthost.invoke("listExtensions"))
  endpoint.handle(HOST_DISPOSE, () => {
    exthost.dispose()
    disposeServices()
    ws.close()
    // disposeServices() schedules the PTY graceful teardown (SIGTERM/taskkill /T)
    // at GRACEFUL_EXIT_DELAY_MS (250ms). Exiting at 0ms killed that timer before
    // it fired — on posix the shells + their children (npm dev servers) orphaned
    // every quit. Wait past the teardown window before exiting.
    setTimeout(() => { process.exit(0) }, EXIT_GRACE_MS)
  })

  // Emit READY the instant the WS server is up — do NOT wait for the child to
  // finish activating extensions. The renderer connects and hydrates the
  // PARENT-owned UI state immediately (sessions, file tree, open folder, PTY),
  // instead of staring at an empty window for seconds while heavy extensions
  // (GitLens et al.) activate in the child. Extensions then stream into the UI
  // via registry/status-bar broadcasts as the child activates in parallel; the
  // ext count starts at 0 and the footer fills in live.
  endpoint.emit(EVT_READY, { extensions: 0, wsPort: ws.port, wsToken: ws.token })
}
