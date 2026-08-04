// B3 integration — drives the real ExtensionLoader against the on-disk
// builtin extensions, proving the activation engine actually activates them
// (and registers their commands) end-to-end, not just in the unit harness.
import { fileURLToPath } from "node:url"
import { describe, expect, it } from "vitest"
import { Diagnostics } from "./api/diagnostics.js"
import { FileDecorations } from "./api/file-decorations.js"
import { LanguageFeatures } from "./api/language-features.js"
import { SessionsApi } from "./api/sessions.js"
import { StatusBar } from "./api/status-bar.js"
import { TreeViews } from "./api/tree-views.js"
import { Webviews } from "./api/webview.js"
import type { EnvHost, SessionsHost, StorageHost, WorkspaceHost } from "./host-services.js"
import { ExtensionLoader } from "./loader.js"
import { Registry } from "./registry.js"

// Relative to THIS file (src/exthost/) → repo's builtin-extensions/, so the
// test doesn't depend on the vitest working directory.
const builtinDir = fileURLToPath(new URL("../../builtin-extensions", import.meta.url))

function stubWorkspaceHost(): WorkspaceHost {
  const noop = (): (() => void) => () => { /* no-op unsubscribe */ }
  return {
    getFolderPath: () => null,
    onDidChangeFolder: noop,
    listFiles: () => Promise.resolve([]),
    watchFiles: noop,
    fs: {
      stat: () => Promise.reject(new Error("unused")),
      readFile: () => Promise.reject(new Error("unused")),
      writeFile: () => Promise.resolve(),
      readDirectory: () => Promise.resolve([]),
      createDirectory: () => Promise.resolve(),
      delete: () => Promise.resolve(),
      rename: () => Promise.resolve(),
      copy: () => Promise.resolve(),
    },
    config: {
      registerDefaults: () => { /* no-op */ },
      getAll: () => ({}),
      inspect: () => ({}),
      update: () => Promise.resolve(),
      onDidChange: noop,
    },
    documents: {
      list: () => [],
      get: () => undefined,
      open: () => { /* no-op */ },
      onDidOpen: noop,
      onDidChange: noop,
      onDidClose: noop,
      onDidSave: noop,
    },
    editors: {
      getActive: () => null,
      getSelections: () => [],
      onDidChangeActive: noop,
      onDidChangeSelection: noop,
      applyEdits: () => Promise.resolve(false),
      revealRange: () => Promise.resolve(),
      showDocument: () => Promise.resolve(),
      setDecorations: noop,
      disposeDecorationType: noop,
      insertSnippet: () => Promise.resolve(false),
      setSelections: noop,
    },
  }
}

function stubStorageHost(): StorageHost {
  return {
    globalGet: () => undefined, globalUpdate: () => { /* no-op */ }, globalKeys: () => [],
    workspaceGet: () => undefined, workspaceUpdate: () => { /* no-op */ }, workspaceKeys: () => [],
    secretGet: () => Promise.resolve(undefined), secretSet: () => Promise.resolve(), secretDelete: () => { /* no-op */ },
    secretKeys: () => [], onSecretChange: () => () => { /* no-op */ },
    globalStorageDir: () => "/tmp/gs", logDir: () => "/tmp/log", storageDir: () => null,
  }
}

function stubEnvHost(): EnvHost {
  return { sync: () => { /* no-op */ }, drop: () => { /* no-op */ } }
}

function stubSessionsHost(): SessionsHost {
  return {
    list: () => ({ projects: [], sessions: [], activeSessionId: null }),
    getActive: () => null,
    onChange: () => ({ dispose() { /* no-op */ } }),
    onActiveChange: () => ({ dispose() { /* no-op */ } }),
    createSession: () => Promise.resolve({ id: "x", name: "x", projectId: "p", lastOpened: 0, createdAt: 0 }),
    setActiveSession: () => { /* no-op */ },
    updateSession: () => Promise.resolve(null),
    deleteSession: () => { /* no-op */ },
  }
}

function makeLoader(registry: Registry): ExtensionLoader {
  return new ExtensionLoader({
    builtinDir,
    // No sideloaded extensions in the unit harness — a non-existent dir scans empty.
    userExtDir: `${builtinDir}__no_user_ext__`,
    registry,
    emitNotification: () => { /* no-op */ },
    showMessage: () => Promise.resolve(undefined),
    showInputBox: () => Promise.resolve(undefined),
    showQuickPick: () => Promise.resolve(null),
    showOpenDialog: () => Promise.resolve(null),
    showSaveDialog: () => Promise.resolve(null),
    openExternal: () => Promise.resolve(true),
    readClipboard: () => Promise.resolve(""),
    emitOutputEvent: () => { /* no-op */ },
    writeClipboard: () => { /* no-op */ },
    workspaceHost: stubWorkspaceHost(),
    storage: stubStorageHost(),
    env: stubEnvHost(),
    languageFeatures: new LanguageFeatures(),
    diagnostics: new Diagnostics(() => { /* no-op */ }),
    webviews: new Webviews(() => { /* no-op */ }),
    treeViews: new TreeViews(() => { /* no-op */ }),
    fileDecorations: new FileDecorations(() => { /* no-op */ }),
    statusBar: new StatusBar(() => { /* no-op */ }),
    sessionsApi: new SessionsApi(stubSessionsHost()),
  })
}

describe("ExtensionLoader activation (B3 integration)", () => {
  // One prepare+activate for the whole suite — the Node require cache + the global
  // vscode-require hook make a second independent prepareAll in the same process
  // re-bind cached extension modules to the first loader's api (prod has a
  // single process-lifetime loader, so this never happens there).
  it("activates builtins, then disables/re-enables one at runtime", async () => {
    const registry = new Registry()
    const loader = makeLoader(registry)
    const descriptors = loader.prepareAll()
    await loader.activateStartup()
    expect(descriptors.length).toBeGreaterThan(0) // builtin-extensions/ found
    const hello = descriptors.find((d) => d.id.endsWith("hello-world"))
    expect(hello?.active).toBe(true)
    expect(hello?.activationError).toBeUndefined()
    expect(registry.hasCommand("hello-world.sayHello")).toBe(true)

    // Disable at runtime → its command registration is disposed live.
    const id = hello?.id ?? ""
    await loader.setExtensionEnabled(id, false)
    expect(registry.hasCommand("hello-world.sayHello")).toBe(false)
    expect(loader.list().find((d) => d.id === id)?.enabled).toBe(false)

    // Re-enable → re-prepared + re-activated, command back.
    await loader.setExtensionEnabled(id, true)
    expect(registry.hasCommand("hello-world.sayHello")).toBe(true)
    expect(loader.list().find((d) => d.id === id)?.enabled).toBe(true)
  })
})
