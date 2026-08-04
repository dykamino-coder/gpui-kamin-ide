// ExtensionContext + ExtensionFacade builders.
//
// Both shapes mirror the d.ts contract that extensions consume via
// `vscode.ExtensionContext` and `vscode.Extension<T>`. Neither has any
// real persistence yet (Phase A skeleton); state is per-process and
// vanishes on reload. Phase B will back globalState/workspaceState
// with electron-store partitions and secrets with `safeStorage`.
//
// The facade uses two off-spec mutators (`__setActive`, `__setExports`)
// stitched onto the public shape — they're exposed via a sibling
// helper so callers don't have to repeat the unsafe cast.
import { join } from "node:path"
import { URI } from "vscode-uri"
import type { Disposable, ExtensionDescriptor } from "../api/types.js"
import { ExtensionMode, ExtensionKind, EnvironmentVariableMutatorType } from "./api/enums.js"
import { Disposable as DisposableImpl, EventEmitter, type EventSignature } from "./api/shared.js"
import type { ExtensionFacade } from "./api/types.js"
import type { EnvHost, EnvVarMutationDto, StorageHost } from "./host-services.js"

// `vscode.EnvironmentVariableCollection` (#11). Mutations are kept here (vscode's
// get/forEach are synchronous) and pushed as a full snapshot to the parent, which
// applies them to integrated-terminal spawns. `persistent` defaults to true.
interface EnvMutatorOptions { applyAtProcessCreation?: boolean; applyAtShellIntegration?: boolean }
// vscode.EnvironmentVariableMutator — fields are readonly in the d.ts.
interface EnvMutator { readonly value: string; readonly type: number; readonly options: EnvMutatorOptions }
// vscode allows `string | MarkdownString` (the latter is `{ value: string }`).
type EnvDescription = string | { value: string }
interface EnvVarCollection {
  persistent: boolean
  description: EnvDescription | undefined
  replace: (variable: string, value: string, options?: EnvMutatorOptions) => void
  append: (variable: string, value: string, options?: EnvMutatorOptions) => void
  prepend: (variable: string, value: string, options?: EnvMutatorOptions) => void
  get: (variable: string) => EnvMutator | undefined
  forEach: (cb: (variable: string, mutator: EnvMutator, collection: EnvVarCollection) => void, thisArg?: unknown) => void
  delete: (variable: string) => void
  clear: () => void
  [Symbol.iterator]: () => IterableIterator<[string, EnvMutator]>
}

/** Flatten a description (string | MarkdownString) to the plain string the DTO carries. */
function descText(v: EnvDescription | undefined): string | undefined {
  return typeof v === "string" ? v : v?.value
}

function makeEnvCollection(env: EnvHost, extId: string): EnvVarCollection {
  const vars = new Map<string, EnvMutator>()
  let persistent = true
  let description: EnvDescription | undefined
  const push = (): void => {
    const out: Record<string, EnvVarMutationDto> = {}
    for (const [k, m] of vars) out[k] = { type: m.type, value: m.value }
    const desc = descText(description)
    env.sync(extId, desc === undefined ? { persistent, vars: out } : { persistent, description: desc, vars: out })
  }
  const setter = (type: number) => (variable: string, value: string, options?: EnvMutatorOptions): void => {
    vars.set(variable, { value, type, options: options ?? { applyAtProcessCreation: true } })
    push()
  }
  const collection: EnvVarCollection = {
    get persistent() { return persistent },
    set persistent(v: boolean) { persistent = v; push() },
    get description() { return description },
    set description(v: EnvDescription | undefined) { description = v; push() },
    replace: setter(EnvironmentVariableMutatorType.Replace),
    append: setter(EnvironmentVariableMutatorType.Append),
    prepend: setter(EnvironmentVariableMutatorType.Prepend),
    get: (variable) => vars.get(variable),
    forEach: (cb, thisArg) => { for (const [k, m] of vars) cb.call(thisArg, k, m, collection) },
    delete: (variable) => { vars.delete(variable); push() },
    clear: () => { vars.clear(); push() }, // d.ts: clears mutators only (not description)
    [Symbol.iterator]: () => vars.entries(),
  }
  return collection
}

export interface ExtensionContextStub {
  subscriptions: Disposable[]
  extensionPath: string
  extensionUri: URI
  globalState: {
    get: <T>(key: string, defaultValue?: T) => T | undefined
    update: (key: string, value: unknown) => Promise<void>
    keys: () => readonly string[]
    setKeysForSync: (keys: readonly string[]) => void
  }
  workspaceState: {
    get: <T>(key: string, defaultValue?: T) => T | undefined
    update: (key: string, value: unknown) => Promise<void>
    keys: () => readonly string[]
  }
  secrets: {
    get: (key: string) => Promise<string | undefined>
    store: (key: string, value: string) => Promise<void>
    delete: (key: string) => Promise<void>
    keys: () => Promise<readonly string[]>
    onDidChange: EventSignature<{ key: string }>
  }
  asAbsolutePath: (rel: string) => string
  logUri: URI
  // undefined when no workspace folder is open (VS Code parity).
  storageUri: URI | undefined
  globalStorageUri: URI
  // Deprecated string (fsPath) variants — still in the d.ts; extensions
  // targeting VS Code 1.50–1.74 read these. Projections of the Uri forms.
  storagePath: string | undefined
  globalStoragePath: string
  logPath: string
  extensionMode: number
  extensionKind: number
  extension: ExtensionFacade
  environmentVariableCollection: EnvVarCollection & { getScoped: (scope: unknown) => EnvVarCollection }
}

export function makeContext(extPath: string, subscriptions: Disposable[], extension: ExtensionFacade, storage: StorageHost, env: EnvHost, extId: string): ExtensionContextStub {
  const extensionUri = URI.file(extPath)
  // B9: persistence dirs live under the host data root (not the read-only,
  // once-VSIX-packaged extension dir). storageUri is undefined with no folder.
  const logUri = URI.file(storage.logDir(extId))
  const globalStorageUri = URI.file(storage.globalStorageDir(extId))
  const wsStorageDir = storage.storageDir(extId)
  const storageUri = wsStorageDir ? URI.file(wsStorageDir) : undefined

  // SecretStorage change events: bridge the host's per-key fire to a vscode
  // Event<{key}>. The subscription is torn down with the extension (unload).
  const secretChange = new EventEmitter<{ key: string }>()
  subscriptions.push(new DisposableImpl(storage.onSecretChange(extId, (key) => { secretChange.fire({ key }) })))

  // environmentVariableCollection (#11): the global collection + isolated scoped
  // collections (getScoped, keyed by workspace-folder uri). Writes to a scoped
  // collection do NOT alias the global one (d.ts isolation contract). All
  // contributions are dropped on disable/unload (no stale terminal PATH).
  const envCollection = makeEnvCollection(env, extId)
  const scoped = new Map<string, EnvVarCollection>()
  const scopedId = (key: string): string => `${extId}::scope::${key}`
  const getScoped = (scope: unknown): EnvVarCollection => {
    const wf = (scope as { workspaceFolder?: { uri?: { toString: () => string } } } | undefined)?.workspaceFolder
    const key = wf?.uri?.toString() ?? "default"
    let c = scoped.get(key)
    if (!c) { c = makeEnvCollection(env, scopedId(key)); scoped.set(key, c) }
    return c
  }
  subscriptions.push(new DisposableImpl(() => {
    env.drop(extId)
    for (const key of scoped.keys()) env.drop(scopedId(key))
  }))

  return {
    subscriptions,
    extensionPath: extPath,
    extensionUri,
    globalState: {
      get: <T>(key: string, def?: T) => { const v = storage.globalGet(extId, key); return (v === undefined ? def : v) as T | undefined },
      update: (key: string, value: unknown) => { storage.globalUpdate(extId, key, value); return Promise.resolve() },
      keys: () => storage.globalKeys(extId),
      // No settings-sync backend — recording sync keys would be inert, so no-op.
      setKeysForSync: () => { /* no-op: no settings sync */ },
    },
    workspaceState: {
      get: <T>(key: string, def?: T) => { const v = storage.workspaceGet(extId, key); return (v === undefined ? def : v) as T | undefined },
      update: (key: string, value: unknown) => { storage.workspaceUpdate(extId, key, value); return Promise.resolve() },
      keys: () => storage.workspaceKeys(extId),
    },
    secrets: {
      get: (key: string) => storage.secretGet(extId, key),
      store: (key: string, value: string) => storage.secretSet(extId, key, value),
      delete: (key: string) => { storage.secretDelete(extId, key); return Promise.resolve() },
      keys: () => Promise.resolve(storage.secretKeys(extId)),
      onDidChange: secretChange.event,
    },
    asAbsolutePath: (rel: string) => join(extPath, rel),
    logUri,
    storageUri,
    globalStorageUri,
    storagePath: storageUri?.fsPath,
    globalStoragePath: globalStorageUri.fsPath,
    logPath: logUri.fsPath,
    extensionMode: ExtensionMode.Production,
    extensionKind: ExtensionKind.UI,
    extension,
    environmentVariableCollection: Object.assign(envCollection, { getScoped }),
  }
}

/** Internal mutators stitched onto the facade. They live off-spec so
 *  the public Extension interface stays clean (d.ts has no
 *  `__setActive`); we cast through `unknown` because the facade IS the
 *  shape we built — TS just doesn't see the augmentation. */
interface FacadeMutators {
  __setActive: (v: boolean) => void
  __setExports: (v: unknown) => void
}

export function makeFacade(descriptor: ExtensionDescriptor, extPath: string, manifest: Record<string, unknown>): ExtensionFacade {
  let isActive = false
  let exports: unknown
  return {
    get id() { return descriptor.id },
    get extensionUri() { return URI.file(extPath) },
    get extensionPath() { return extPath },
    get isActive() { return isActive },
    get packageJSON() { return manifest },
    get extensionKind() { return ExtensionKind.UI },
    get exports() { return exports },
    activate: () => Promise.resolve(exports),
    __setActive: (v: boolean) => { isActive = v },
    __setExports: (v: unknown) => { exports = v },
  } as ExtensionFacade & FacadeMutators
}

export function facadeSetActive(facade: ExtensionFacade, active: boolean): void {
  ;(facade as unknown as FacadeMutators).__setActive(active)
}

export function facadeSetExports(facade: ExtensionFacade, exports: unknown): void {
  ;(facade as unknown as FacadeMutators).__setExports(exports)
}
