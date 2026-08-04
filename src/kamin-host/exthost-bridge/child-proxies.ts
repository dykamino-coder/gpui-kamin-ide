// Host-service proxies for the ext-host CHILD. The child runs the pure
// fs/compute services that only back extensions (config, storage, documents,
// editors) IN-PROCESS, exactly as the in-process host did — so those wire
// straight to the service modules. The shared, parent-owned surfaces (raw fs,
// the file index, sessions, the renderer link) cross the fork boundary as RPC.
import type { KaminSession, SessionsSnapshot } from "../../api/types.js"
import type { EnvCollectionSnapshot, EnvHost, SessionsHost, StorageHost, WatchEvent, WorkspaceHost } from "../../exthost/host-services.js"
import * as config from "../services/config.js"
import * as documents from "../services/documents.js"
import * as editors from "../services/editors.js"
import { sealSecret, type SecretRelay, unsealSecret } from "../services/secret-crypto.js"
import * as storage from "../services/storage.js"
import { getWorkspaceFolder, onWorkspaceChange } from "../services/workspace.js"
import { HOST_ENV, HOST_FS, HOST_LIST_FILES, HOST_SESSION } from "./protocol.js"

/** Capabilities the child orchestrator threads into the proxies. */
export interface ChildDeps {
  /** RPC to the parent process (host services it owns). */
  call: <T>(method: string, ...params: unknown[]) => Promise<T>
  /** host→renderer request relay (show*, editor writes, secret seal/unseal). */
  requestRenderer: SecretRelay
  /** Local fs-watch fan-out, fed by the parent's MIRROR_WATCH pushes. */
  watchFiles: (fn: (events: readonly WatchEvent[]) => void) => () => void
  /** Sessions mirror, fed by the parent's MIRROR_SESSIONS* pushes. */
  sessions: {
    snapshot: () => SessionsSnapshot
    active: () => KaminSession | null
    onChange: (fn: (s: SessionsSnapshot) => void) => { dispose(): void }
    onActiveChange: (fn: (s: KaminSession | null) => void) => { dispose(): void }
  }
}

export function buildWorkspaceHost(deps: ChildDeps): WorkspaceHost {
  const { call, requestRenderer } = deps
  return {
    getFolderPath: () => getWorkspaceFolder().path,
    onDidChangeFolder: (fn) => onWorkspaceChange(fn),
    fs: {
      stat: (p) => call(HOST_FS, "stat", p),
      readFile: (p) => call(HOST_FS, "readFile", p),
      writeFile: (p, data) => call(HOST_FS, "writeFile", p, data),
      readDirectory: (p) => call(HOST_FS, "readDirectory", p),
      createDirectory: (p) => call(HOST_FS, "createDirectory", p),
      delete: (p, recursive, useTrash) => call(HOST_FS, "delete", p, recursive, useTrash),
      rename: (src, dst, overwrite) => call(HOST_FS, "rename", src, dst, overwrite),
      copy: (src, dst, overwrite) => call(HOST_FS, "copy", src, dst, overwrite),
    },
    listFiles: () => call(HOST_LIST_FILES),
    watchFiles: (fn) => deps.watchFiles(fn),
    config: {
      registerDefaults: (values) => { config.registerConfigDefaults(values) },
      getAll: () => config.getAllConfig(),
      inspect: (key) => config.inspectConfig(key),
      update: (key, value, target) => config.updateConfig(key, value, target),
      onDidChange: (fn) => config.onConfigChange(fn),
    },
    documents: {
      list: () => documents.listDocs(),
      get: (uri) => documents.getDoc(uri),
      open: (doc) => { documents.syncDocOpen(doc) },
      onDidOpen: (fn) => documents.onDocOpen(fn),
      onDidChange: (fn) => documents.onDocChange(fn),
      onDidClose: (fn) => documents.onDocClose(fn),
      onDidSave: (fn) => documents.onDocSave(fn),
    },
    editors: {
      getActive: () => editors.getActiveEditor(),
      getSelections: (uri) => editors.getEditorSelections(uri),
      onDidChangeActive: (fn) => editors.onActiveEditorChange(fn),
      onDidChangeSelection: (fn) => editors.onEditorSelectionChange(fn),
      applyEdits: (uri, edits, eol) => requestRenderer<boolean>("kamin:editor:applyEdits", uri, edits, eol),
      revealRange: (uri, range, revealType) => requestRenderer<undefined>("kamin:editor:revealRange", uri, range, revealType),
      showDocument: (uri) => requestRenderer<undefined>("kamin:editor:show", uri),
      setDecorations: (uri, key, options, items) => { void requestRenderer("kamin:editor:setDecorations", uri, key, options, items) },
      disposeDecorationType: (key) => { void requestRenderer("kamin:editor:disposeDecorationType", key) },
      insertSnippet: (uri, snippet, location) => requestRenderer<boolean>("kamin:editor:insertSnippet", uri, snippet, location),
      setSelections: (uri, selections) => { void requestRenderer("kamin:editor:setSelections", uri, selections) },
    },
  }
}

/** Identical to the in-process storage host: the storage service is a child
 *  singleton (it has its own fs); only secret seal/unseal relays to the renderer. */
export function buildStorageHost(requestRenderer: SecretRelay): StorageHost {
  return {
    globalGet: (extId, key) => storage.globalGet(extId, key),
    globalUpdate: (extId, key, value) => { storage.globalUpdate(extId, key, value) },
    globalKeys: (extId) => storage.globalKeys(extId),
    workspaceGet: (extId, key) => storage.workspaceGet(extId, key),
    workspaceUpdate: (extId, key, value) => { storage.workspaceUpdate(extId, key, value) },
    workspaceKeys: (extId) => storage.workspaceKeys(extId),
    secretGet: async (extId, key) => {
      const stored = storage.secretGet(extId, key)
      return stored === undefined ? undefined : await unsealSecret(requestRenderer, stored)
    },
    secretSet: async (extId, key, value) => { storage.secretSet(extId, key, await sealSecret(requestRenderer, value)) },
    secretDelete: (extId, key) => { storage.secretDelete(extId, key) },
    secretKeys: (extId) => storage.secretKeys(extId),
    onSecretChange: (extId, fn) => storage.onSecretChange(extId, fn),
    globalStorageDir: (extId) => storage.globalStorageDir(extId),
    logDir: (extId) => storage.logDir(extId),
    storageDir: (extId) => storage.storageDir(extId),
  }
}

/** environmentVariableCollection lives in the parent (pty.ts applies it to
 *  terminal spawns); the child pushes a full snapshot on every change. */
export function buildEnvHost(deps: ChildDeps): EnvHost {
  return {
    sync: (extId, snapshot: EnvCollectionSnapshot) => { void deps.call(HOST_ENV, "sync", extId, snapshot) },
    drop: (extId) => { void deps.call(HOST_ENV, "drop", extId) },
  }
}

/** Sessions live in the parent (the renderer drives them too): reads come from
 *  the local mirror, value-returning mutators round-trip over RPC. */
export function buildSessionsHost(deps: ChildDeps): SessionsHost {
  const { call } = deps
  return {
    list: () => deps.sessions.snapshot(),
    getActive: () => deps.sessions.active(),
    onChange: (fn) => deps.sessions.onChange(fn),
    onActiveChange: (fn) => deps.sessions.onActiveChange(fn),
    createSession: (opts) => call<KaminSession>(HOST_SESSION, "create", opts.projectId, opts.name),
    setActiveSession: (id) => { void call(HOST_SESSION, "setActive", id) },
    updateSession: (id, patch) => call<KaminSession | null>(HOST_SESSION, "update", id, patch),
    deleteSession: (id) => { void call(HOST_SESSION, "delete", id) },
  }
}
