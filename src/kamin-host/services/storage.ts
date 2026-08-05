// Extension persistence (B9) backing ExtensionContext.globalState /
// workspaceState / secrets + the storage/global-storage/log directories.
//
// Layout under <dataDir>:
//   global-state.json     { byExt: { <extId>: { <key>: value } } }     (all windows)
//   workspace-state.json  { byFolder: { <hash>: { <extId>: {…} } } }   (per open folder)
//   secrets.json          { byExt: { <extId>: { <key>: string } } }    (DPAPI-sealed — see note)
//   globalStorage/<extId>/        — context.globalStorageUri (always present)
//   logs/<extId>/                 — context.logUri (always present)
//   workspaceStorage/<hash>/<extId>/ — context.storageUri (only with a folder open)
//
// SECURITY NOTE: VS Code encrypts secrets at rest (platform keychain /
// keytar). This store holds whatever string it is handed; the kamin-host
// adapter seals values via the Rust shell's DPAPI (services/secret-crypto.ts)
// before they reach here, so on Windows the bytes on disk are `dpapi:<hex>`
// ciphertext tied to the current user. Values without that marker are legacy /
// fallback plaintext (sealed before this landed, or DPAPI unavailable).
import { createHash } from "node:crypto"
import { mkdirSync } from "node:fs"
import { join } from "node:path"
import { JsonStore } from "../json-store.js"
import { getWorkspaceFolder } from "./workspace.js"

type Kv = Record<string, unknown>
interface ExtStates { byExt: Record<string, Kv> }
interface WsStates { byFolder: Record<string, Record<string, Kv>> }
interface SecretStates { byExt: Record<string, Record<string, string>> }
/** One open webview panel, persisted for WebviewPanelSerializer cross-restart. */
export interface PersistedWebviewPanel { viewType: string; title: string; state: unknown }
/** Scoped per open folder (a panel from project A must not restore in B). The
 *  "" key holds panels opened with no folder open. */
interface WebviewPanelsState { byFolder: Record<string, PersistedWebviewPanel[]> }

const HASH_LEN = 16

let dataRoot = ""
let globalStore: JsonStore<ExtStates> | null = null
let wsStore: JsonStore<WsStates> | null = null
let secretStore: JsonStore<SecretStates> | null = null
let webviewPanelStore: JsonStore<WebviewPanelsState> | null = null
// workspaceState with no folder open → an in-memory, non-persisted scope (an
// "empty workspace" Memento that still works for the session).
const noFolderWs = new Map<string, Kv>()
const secretListeners = new Map<string, Set<(key: string) => void>>()

export function initStorage(dataDir: string): void {
  dataRoot = dataDir
  globalStore = new JsonStore<ExtStates>(dataDir, "global-state", { byExt: {} })
  wsStore = new JsonStore<WsStates>(dataDir, "workspace-state", { byFolder: {} })
  secretStore = new JsonStore<SecretStates>(dataDir, "secrets", { byExt: {} })
  webviewPanelStore = new JsonStore<WebviewPanelsState>(dataDir, "webview-panels", { byFolder: {} })
}

// ── webview panels (WebviewPanelSerializer cross-restart, per-folder) ────
export function loadWebviewPanels(): PersistedWebviewPanel[] {
  return webviewPanelStore?.get().byFolder[folderKey() ?? ""] ?? []
}
export function saveWebviewPanels(panels: PersistedWebviewPanel[]): void {
  const all = webviewPanelStore?.get().byFolder ?? {}
  webviewPanelStore?.set({ byFolder: { ...all, [folderKey() ?? ""]: panels } })
}

/** Stable short hash of the open folder, or null when no folder is open. */
function folderKey(): string | null {
  const p = getWorkspaceFolder().path
  return p ? createHash("sha1").update(p).digest("hex").slice(0, HASH_LEN) : null
}

/** extId is `publisher.name`; keep it filesystem-safe for the dir layout. */
function safeId(extId: string): string {
  return extId.replace(/[^\w.-]/g, "_")
}

/** Apply one Memento write to a copied key-value map (`undefined` removes). */
function writeKv(kv: Kv, key: string, value: unknown): Kv {
  if (value === undefined) {
    const { [key]: _omit, ...rest } = kv
    return rest
  }
  return { ...kv, [key]: value }
}

// ── globalState (all windows) ────────────────────────────────
export function globalGet(extId: string, key: string): unknown {
  return globalStore?.get().byExt[extId]?.[key]
}
export function globalKeys(extId: string): readonly string[] {
  return Object.keys(globalStore?.get().byExt[extId] ?? {})
}
export function globalUpdate(extId: string, key: string, value: unknown): void {
  const all = globalStore?.get().byExt ?? {}
  globalStore?.set({ byExt: { ...all, [extId]: writeKv(all[extId] ?? {}, key, value) } })
}

// ── workspaceState (per open folder; in-memory when none) ─────
export function workspaceGet(extId: string, key: string): unknown {
  const fk = folderKey()
  if (!fk) return noFolderWs.get(extId)?.[key]
  return wsStore?.get().byFolder[fk]?.[extId]?.[key]
}
export function workspaceKeys(extId: string): readonly string[] {
  const fk = folderKey()
  if (!fk) return Object.keys(noFolderWs.get(extId) ?? {})
  return Object.keys(wsStore?.get().byFolder[fk]?.[extId] ?? {})
}
export function workspaceUpdate(extId: string, key: string, value: unknown): void {
  const fk = folderKey()
  if (!fk) {
    noFolderWs.set(extId, writeKv(noFolderWs.get(extId) ?? {}, key, value))
    return
  }
  const all = wsStore?.get().byFolder ?? {}
  const folder = { ...(all[fk] ?? {}) }
  folder[extId] = writeKv(folder[extId] ?? {}, key, value)
  wsStore?.set({ byFolder: { ...all, [fk]: folder } })
}

// ── secrets (PLAINTEXT — see file header) ────────────────────
export function secretGet(extId: string, key: string): string | undefined {
  return secretStore?.get().byExt[extId]?.[key]
}
export function secretKeys(extId: string): readonly string[] {
  return Object.keys(secretStore?.get().byExt[extId] ?? {})
}
export function secretSet(extId: string, key: string, value: string): void {
  const all = secretStore?.get().byExt ?? {}
  secretStore?.set({ byExt: { ...all, [extId]: { ...(all[extId] ?? {}), [key]: value } } })
  fireSecret(extId, key)
}
export function secretDelete(extId: string, key: string): void {
  const all = secretStore?.get().byExt ?? {}
  const { [key]: _omit, ...rest } = all[extId] ?? {}
  secretStore?.set({ byExt: { ...all, [extId]: rest } })
  fireSecret(extId, key)
}
export function onSecretChange(extId: string, listener: (key: string) => void): () => void {
  let set = secretListeners.get(extId)
  if (!set) { set = new Set(); secretListeners.set(extId, set) }
  set.add(listener)
  return () => { set.delete(listener) }
}
function fireSecret(extId: string, key: string): void {
  for (const fn of secretListeners.get(extId) ?? []) fn(key)
}

// ── storage directories ──────────────────────────────────────
function ensureDir(p: string): string {
  mkdirSync(p, { recursive: true })
  return p
}
/** context.globalStorageUri — always available. */
export function globalStorageDir(extId: string): string {
  return ensureDir(join(dataRoot, "globalStorage", safeId(extId)))
}
/** context.logUri — always available. */
export function logDir(extId: string): string {
  return ensureDir(join(dataRoot, "logs", safeId(extId)))
}
/** context.storageUri — null when no workspace folder is open (VS Code parity). */
export function storageDir(extId: string): string | null {
  const fk = folderKey()
  return fk ? ensureDir(join(dataRoot, "workspaceStorage", fk, safeId(extId))) : null
}
