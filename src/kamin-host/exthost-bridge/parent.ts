// Parent side of the ext-host split. Forks the child (`--role=exthost`),
// supervises it (a native crash → unexpected exit → respawn), serves the
// host-service RPC the child needs, and pushes mirror deltas. Exposes `invoke`
// so the renderer-facing method table routes calls into the child, plus a
// `ready` promise the boot sequence waits on.
import { type ChildProcess, execFile, fork } from "node:child_process"
import type { KaminSession, SessionsSnapshot } from "../../api/types.js"
import type { EnvCollectionSnapshot, HostFs, IndexedFileRef, WatchEvent } from "../../exthost/host-services.js"
import { createHostIncidentLogs } from "../incident-log.js"
import type { MessagePortLike } from "../port.js"
import { SLOW_FRAME_MS, isRpcFrame } from "../protocol.js"
import { RpcEndpoint } from "../rpc.js"
import { dropEnvCollection, syncEnvCollection } from "../services/env-collection.js"
import {
  CHILD_INVOKE, CHILD_READY,
  EXTHOST_ROLE_ARG, EXTHOST_ROLE_VALUE,
  type ExtHostSeed, type HostEnvOp, type HostFsOp, type HostSessionOp,
  HOST_BROADCAST, HOST_ENV, HOST_FS, HOST_LIST_FILES, HOST_REQUEST_RENDERER, HOST_SEED, HOST_SESSION,
  MIRROR_SESSIONS, MIRROR_SESSIONS_ACTIVE, MIRROR_WATCH, MIRROR_WORKSPACE,
} from "./protocol.js"

// Respawn backoff — index by consecutive-crash count (reset on CHILD_READY).
// A child that boots fine then crashes once respawns in 200 ms; a child that
// crashes repeatedly without ever activating (bad seed, corrupt state) backs
// off and finally gives up rather than spin forever flooding the user.
// eslint-disable-next-line no-magic-numbers -- the ladder of ms delays IS the schedule
const RESPAWN_BACKOFF_MS = [200, 1000, 5000, 15_000]
const RESPAWN_GIVE_UP = 6

/** Kill an exthost child AND its descendants (the language servers extensions
 *  fork — gopls, rust-analyzer, volar…). Without this, a crash-respawn stacks a
 *  fresh generation of LS processes on top of the orphaned old ones (up to
 *  RESPAWN_GIVE_UP generations, each hundreds of MB); the app-level Job Object
 *  only reaps them on full exit. Best-effort + fire-and-forget. */
function killProcessTree(pid: number | undefined): void {
  if (!pid) return
  if (process.platform === "win32") {
    try { execFile("taskkill", ["/F", "/T", "/PID", String(pid)], { windowsHide: true }, () => { /* ignore */ }) } catch { /* ignore */ }
  } else {
    try { process.kill(-pid, "SIGKILL") } catch { try { process.kill(pid, "SIGKILL") } catch { /* gone */ } }
  }
}

export interface ParentDeps {
  dataDir: string
  builtinDir: string
  userExtDir: string
  /** ws.broadcast — host→renderer fire-and-forget. */
  broadcast: (channel: string, payload: unknown) => void
  /** ws.request — host→renderer request/response. */
  request: <T>(method: string, ...params: unknown[]) => Promise<T>
  fs: HostFs
  listFiles: () => readonly IndexedFileRef[] | Promise<readonly IndexedFileRef[]>
  workspace: { getFolderPath: () => string | null; onChange: (fn: (p: string | null) => void) => void }
  sessions: {
    list: () => SessionsSnapshot
    getActive: () => KaminSession | null
    onChange: (fn: (s: SessionsSnapshot) => void) => void
    onActiveChange: (fn: (s: KaminSession | null) => void) => void
    create: (projectId?: string, name?: string) => KaminSession
    setActive: (id: string) => void
    update: (id: string, patch: { name?: string; metadata?: Record<string, unknown> }) => KaminSession | null
    delete: (id: string) => void
  }
  watch: { subscribe: (fn: (events: readonly WatchEvent[]) => void) => void }
}

export interface ExtHostHandle {
  /** Route a renderer-facing call into the child (returns its result). */
  invoke: <T>(method: string, ...params: unknown[]) => Promise<T>
  dispose: () => void
}

function childPort(child: ChildProcess): MessagePortLike {
  return {
    // Guard `connected`: a send after the child died (crash → respawn window)
    // would otherwise emit an IPC 'error' event that, unhandled, crashes US.
    post: (frame) => { if (child.connected) child.send(frame) },
    onFrame: (fn) => {
      child.on("message", (m: unknown) => {
        if (!isRpcFrame(m)) return
        // Замер обработки кадра от ребёнка: сюда попадает и структурированный
        // клон мегабайтных полезных нагрузок (HTML вью), и вся дальнейшая
        // синхронная работа хендлера.
        const t0 = Date.now()
        fn(m)
        const dt = Date.now() - t0
        if (dt >= SLOW_FRAME_MS) {
          const kind = (m as { method?: string }).method ?? "res"
          console.error(`[child frame] ${kind} ${dt}ms`)
        }
      })
    },
  }
}

export function forkExtHost(deps: ParentDeps): ExtHostHandle {
  let endpoint: RpcEndpoint | null = null
  let child: ChildProcess | null = null
  let disposed = false
  let respawnAttempts = 0
  // Monotonic for the lifetime of the supervising host. Unlike a timestamp
  // created inside the child, this survives extension-host crashes and cannot
  // move backwards after a wall-clock/NTP adjustment.
  let childGeneration = 0
  // True once the child has signalled CHILD_READY at least once; a later ready is
  // a respawn-after-crash, which needs the renderer to re-seed its mirror.
  let everReady = false
  // Per-GENERATION readiness. `endpoint` exists the instant the port connects,
  // but the child registers its CHILD_INVOKE dispatch table only after its async
  // loadAll — so an invoke fired in that gap (a renderer restore right after an
  // OOM reload) reaches the child endpoint with no handler and rejects with the
  // raw "unknown method: exthost:invoke". Gate every invoke on the child's own
  // CHILD_READY signal instead of on endpoint-existence. Waiters resolve on the
  // next ready (covering a respawn in flight) or reject after READY_WAIT_MS.
  let ready = false
  let readyWaiters: ((err: Error | null) => void)[] = []
  // A pure BACKSTOP, not a per-call timeout — the transport is timeout-free by
  // policy (see rpc.ts). Give-up and dispose reject waiters promptly; this only
  // catches a child that spawned but never signals CHILD_READY (stuck module
  // load), so it's generous enough that a cold fork + module load + HOST_SEED on
  // a slow/AV-scanned disk can't false-fire.
  const READY_WAIT_MS = 30_000
  const signalReady = (): void => {
    ready = true
    const pending = readyWaiters
    readyWaiters = []
    for (const w of pending) w(null)
  }
  // Reject every pending whenReady() NOW (respawn give-up / dispose) — mirrors
  // endpoint.failAll for the CHILD_INVOKE queue, so a caller fails fast with the
  // real reason instead of waiting out its individual backstop.
  const failReadyWaiters = (reason: string): void => {
    const pending = readyWaiters
    readyWaiters = []
    for (const w of pending) w(new Error(reason))
  }
  const whenReady = (): Promise<void> => {
    // disposed FIRST: dispose() flips it synchronously but `ready`/`endpoint`
    // only clear later on the child's exit event — an invoke landing in that gap
    // must not take the fast path into a child being torn down.
    if (disposed) return Promise.reject(new Error("exthost disposed"))
    if (ready && endpoint) return Promise.resolve()
    return new Promise<void>((resolve, reject) => {
      let settled = false
      const settle = (err: Error | null): void => {
        if (settled) return
        settled = true
        clearTimeout(t)
        if (err) reject(err); else resolve()
      }
      const t = setTimeout(() => { settle(new Error("exthost child not ready (timeout)")) }, READY_WAIT_MS)
      readyWaiters.push(settle)
    })
  }

  const seed = (): ExtHostSeed => ({
    dataDir: deps.dataDir,
    builtinDir: deps.builtinDir,
    userExtDir: deps.userExtDir,
    workspaceFolder: deps.workspace.getFolderPath(),
    sessions: { snapshot: deps.sessions.list(), active: deps.sessions.getActive() },
  })

  const onFs = (op: HostFsOp, a: unknown[]): unknown => {
    const p = a[0] as string
    switch (op) {
      case "stat": return deps.fs.stat(p)
      case "readFile": return deps.fs.readFile(p)
      case "writeFile": return deps.fs.writeFile(p, a[1] as Uint8Array)
      case "readDirectory": return deps.fs.readDirectory(p)
      case "createDirectory": return deps.fs.createDirectory(p)
      case "delete": return deps.fs.delete(p, a[1] as boolean, a[2] as boolean)
      case "rename": return deps.fs.rename(p, a[1] as string, a[2] as boolean)
      case "copy": return deps.fs.copy(p, a[1] as string, a[2] as boolean)
      default: throw new Error(`exthost bridge: unknown fs op "${String(op)}"`)
    }
  }

  const onSession = (op: HostSessionOp, a: unknown[]): unknown => {
    switch (op) {
      case "create": return deps.sessions.create(a[0] as string | undefined, a[1] as string | undefined)
      case "setActive": deps.sessions.setActive(a[0] as string); return
      case "update": return deps.sessions.update(a[0] as string, a[1] as { name?: string; metadata?: Record<string, unknown> })
      case "delete": deps.sessions.delete(a[0] as string); return
      default: throw new Error(`exthost bridge: unknown session op "${String(op)}"`)
    }
  }

  const onEnv = (op: HostEnvOp, a: unknown[]): void => {
    const extId = a[0] as string
    if (op === "sync") syncEnvCollection(extId, a[1] as EnvCollectionSnapshot)
    else dropEnvCollection(extId)
  }

  const serve = (ep: RpcEndpoint): void => {
    ep.handle(HOST_SEED, () => seed())
    ep.handle(HOST_FS, (op, ...a) => onFs(op as HostFsOp, a))
    ep.handle(HOST_LIST_FILES, () => deps.listFiles())
    ep.handle(HOST_SESSION, (op, ...a) => onSession(op as HostSessionOp, a))
    ep.handle(HOST_ENV, (op, ...a) => { onEnv(op as HostEnvOp, a) })
    ep.handle(HOST_REQUEST_RENDERER, (method, params) => deps.request(method as string, ...(params as unknown[])))
    ep.onEvent((channel, payload) => {
      if (channel === HOST_BROADCAST) {
        const b = payload as { channel: string; payload: unknown }
        deps.broadcast(b.channel, b.payload)
      } else if (channel === CHILD_READY) {
        respawnAttempts = 0 // a clean activation clears the crash-loop backoff
        // A RE-ready (after a crash) means the child's mirror/registry restarted
        // empty — tell the renderer to re-seed open docs + rebuild its views.
        // (Not on first boot: the renderer's initial sync already covers that.)
        if (everReady) deps.broadcast("kamin:exthost:respawned", null)
        everReady = true
        console.error(`[boot] CHILD_READY через ${Date.now() - tFork}ms после форка`)
        signalReady() // this generation's dispatch table is live — release invokes
      }
    })
  }

  const pushMirror = (channel: string, payload: unknown): void => { endpoint?.emit(channel, payload) }

  // Persist the child's stdout/stderr (extension activation stacks, console.*)
  // to a log file, like VS Code's extension-host log — the GUI shell swallows
  // stderr, so this is the only way to read activation failures post-mortem.
  const logs = createHostIncidentLogs(deps.dataDir)

  const selfScript = process.argv[1] ?? ""
  // Диагностика бюджета: сколько живёт форк до сигнала готовности. Родитель
  // держит ВСЕ invoke до CHILD_READY, поэтому это время = задержка панелей.
  let tFork = 0
  const spawn = (): void => {
    ready = false // fresh generation is not invocable until it signals CHILD_READY
    childGeneration += 1
    tFork = Date.now()
    console.error(`[boot] fork exthost: ${selfScript} execArgv=${JSON.stringify(process.execArgv)}`)
    const proc = fork(selfScript, [`--${EXTHOST_ROLE_ARG}=${EXTHOST_ROLE_VALUE}`], {
      silent: true,
      // `advanced` (structured clone) IPC, not the default `json`: extensions
      // round-trip binary data through vscode.workspace.fs (Uint8Array). With
      // json serialization a Buffer/Uint8Array degrades to a plain object, so an
      // extension writing those bytes downstream (e.g. golang.Go piping file
      // contents into gopls' stdin) throws "data must be a string or Buffer".
      serialization: "advanced",
      // posix: own process group so process.kill(-pid) reaps forked language
      // servers too. (Windows uses taskkill /T by ppid, no group needed.) NOT
      // unref'd — we still supervise it.
      detached: process.platform !== "win32",
      env: {
        ...process.env,
        KAMIN_HOST_TRANSPORT: "ipc",
        KAMIN_EXTHOST_GENERATION: String(childGeneration),
      },
    })
    child = proc
    // Child stdout/stderr → our stderr so its logs + crash stacks reach the
    // Rust shell's tracing (the child has no console of its own).
    proc.stdout?.on("data", (d: Buffer) => { process.stderr.write(d); logs.writeRaw(d) })
    proc.stderr?.on("data", (d: Buffer) => { process.stderr.write(d); logs.writeRaw(d) })
    // Swallow IPC errors (EPIPE / channel-closed on a dying child) — the `exit`
    // handler owns recovery; an unhandled 'error' here would crash the parent.
    proc.on("error", (e) => { console.error("exthost child: ipc error", e.message) })
    endpoint = new RpcEndpoint(childPort(proc))
    serve(endpoint)
    proc.on("exit", (code, signal) => {
      logs.writeExtensionExit(proc.pid ?? null, code, signal ?? null)
      // Null the endpoint FIRST so calls racing into the respawn window reject
      // promptly ("child not ready") instead of queueing on the dead endpoint
      // (whose guarded post drops the frame, hanging the promise forever).
      endpoint?.failAll("exthost child exited")
      endpoint = null
      ready = false // next generation must re-signal CHILD_READY before invokes fire
      // Reap any language servers this generation forked before respawning, so
      // crash-loops don't stack generations of gopls/rust-analyzer.
      killProcessTree(proc.pid)
      if (disposed) return
      respawnAttempts += 1
      if (respawnAttempts > RESPAWN_GIVE_UP) {
        failReadyWaiters("extension host disabled") // no child is coming — fail waiters now
        deps.broadcast("kamin:notification:show", {
          severity: "error", title: "Extension host disabled",
          message: "The extension host crashed repeatedly and was not restarted. Reload the window to retry.",
          timestamp: Date.now(),
        })
        return
      }
      deps.broadcast("kamin:notification:show", {
        severity: "error", title: "Extension host restarted",
        message: `The extension host process exited (code ${String(code)}) and was restarted.`,
        timestamp: Date.now(),
      })
      const delay = RESPAWN_BACKOFF_MS[Math.min(respawnAttempts - 1, RESPAWN_BACKOFF_MS.length - 1)]
      setTimeout(spawn, delay)
    })
  }

  // Mirror subscriptions wired once; they push to whichever child is current.
  deps.workspace.onChange((p) => { pushMirror(MIRROR_WORKSPACE, p) })
  deps.sessions.onChange((s) => { pushMirror(MIRROR_SESSIONS, s) })
  deps.sessions.onActiveChange((s) => { pushMirror(MIRROR_SESSIONS_ACTIVE, s) })
  deps.watch.subscribe((b) => { pushMirror(MIRROR_WATCH, b) })

  spawn()

  return {
    invoke: async <T,>(method: string, ...params: unknown[]): Promise<T> => {
      await whenReady() // block until the child's dispatch table is live (or time out)
      if (!endpoint) throw new Error("exthost child not ready")
      return await endpoint.call<T>(CHILD_INVOKE, method, ...params)
    },
    dispose: () => {
      disposed = true
      failReadyWaiters("exthost disposed")
      killProcessTree(child?.pid)
      child?.kill()
      logs.close()
    },
  }
}
