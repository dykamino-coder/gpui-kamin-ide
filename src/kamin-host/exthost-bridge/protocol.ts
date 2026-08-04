import type { KaminSession, SessionsSnapshot } from "../../api/types.js"

// Wire contract between the kamin-host PARENT (user-facing services + renderer
// WS link) and the forked ext-host CHILD (extension code). Both run the SAME
// bundle — the child is `node kamin-host.mjs --role=exthost` — so this one
// module fixes the shape for both sides. Frames ride the fork IPC channel via
// RpcEndpoint (structured clone preserves Uint8Array, unlike the stdio-JSON
// Rust link the parent uses upstream).
//
// The split exists for native-crash isolation: a segfault in an extension's
// native addon kills only the child; the parent (PTY, fs, index, renderer WS)
// survives and respawns it. JS-level uncaught throws stay contained in-process
// by the child's own handler (the Step-1 policy, now living in the child).
//
// Division of labour — the child is a full Node process with its own fs, so the
// pure fs/compute services that ONLY back the extension host (config, storage,
// documents, editors) RUN IN THE CHILD. Their one coupling to the host (config
// + storage follow the open folder) is a single string, supplied by the
// workspace-folder mirror. The parent keeps what the renderer also needs (fs
// ops, file index, sessions, real workspace persistence, PTY); the child reaches
// those over RPC. Mirrored shared state: workspace folder, sessions, fs-watch.

/** The argv flag the parent forks the child with (`--role=exthost`). */
export const EXTHOST_ROLE_ARG = "role"
export const EXTHOST_ROLE_VALUE = "exthost"

// ─── parent → child ────────────────────────────────────────────────
/** Generic dispatch carrying every renderer-facing call routed to the child:
 *  ExtHost methods (snapshot/executeCommand/treeGetChildren/provideHover/…) AND
 *  the renderer's Monaco doc/editor sync (doc:open/change/close/save,
 *  editor:active/selections). One boundary, a method→handler map on the child. */
export const CHILD_INVOKE = "exthost:invoke"
/** Child signals it has booted + activated its extensions. */
export const CHILD_READY = "exthost:ready"

// Mirror pushes (fire-and-forget) keep the child's small shared-state copies
// fresh so the synchronous vscode getters read without a round-trip.
export const MIRROR_WORKSPACE = "mirror:workspace" // (path: string | null)
export const MIRROR_SESSIONS = "mirror:sessions" // (SessionsSnapshot)
export const MIRROR_SESSIONS_ACTIVE = "mirror:sessions:active" // (KaminSession | null)
export const MIRROR_WATCH = "mirror:watch" // (WatchEvent[])

// ─── child → parent ────────────────────────────────────────────────
/** Initial shared state the child seeds at boot (ExtHostSeed). */
export const HOST_SEED = "host:seed"
/** Fire-and-forget renderer broadcast → parent `ws.broadcast(channel, payload)`. */
export const HOST_BROADCAST = "host:broadcast"
/** host→renderer request/response → parent `ws.request(method, ...params)`.
 *  Covers show*Message/InputBox/QuickPick, editor write-backs, and the secret
 *  seal/unseal DPAPI relay — every child path that ends at the renderer. */
export const HOST_REQUEST_RENDERER = "host:requestRenderer"
/** Raw fs op the parent owns (renderer shares it): (op, ...args). */
export const HOST_FS = "host:fs"
/** Workspace file index for `findFiles` — async, parent-owned. */
export const HOST_LIST_FILES = "host:listFiles"
/** Sessions mutator the parent owns (renderer shares it): (op, ...args). */
export const HOST_SESSION = "host:session"
/** environmentVariableCollection sync — the parent applies it to terminal
 *  spawns (pty.ts lives there): (op, extId, snapshot?). */
export const HOST_ENV = "host:env"

/** ops for the multiplexed HOST_ENV channel. */
export type HostEnvOp = "sync" | "drop"

/** ops for the multiplexed HOST_FS channel (mirror of HostFs). */
export type HostFsOp =
  | "stat" | "readFile" | "writeFile" | "readDirectory"
  | "createDirectory" | "delete" | "rename" | "copy"

/** ops for the multiplexed HOST_SESSION channel (mirror of SessionsHost mutators). */
export type HostSessionOp = "create" | "setActive" | "update" | "delete"

/** Seed payload returned by HOST_SEED — the child roots / mirrors from this. */
export interface ExtHostSeed {
  dataDir: string
  builtinDir: string
  userExtDir: string
  workspaceFolder: string | null
  sessions: { snapshot: SessionsSnapshot; active: KaminSession | null }
}
