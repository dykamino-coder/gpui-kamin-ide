// The single `acquireVsCodeApi()` handle for the whole webview. VS Code (and
// KaminIDE) allow it to be called EXACTLY ONCE per webview — everything that
// needs postMessage or persisted state imports it from here. In a non-webview
// context (e.g. a unit test) it falls back to an in-memory stub.
export interface WebviewApi {
  postMessage: (msg: unknown) => void
  getState: () => unknown
  setState: (state: unknown) => void
  /** Browser sockets deliver through a private subscription, never window messages. */
  onHostMessage?: (receive: (frame: unknown) => void) => () => void
  onDisconnect?: (receive: () => void) => () => void
}

const acquire = (globalThis as { acquireVsCodeApi?: () => WebviewApi }).acquireVsCodeApi
let browserInstalled = false
function resolve(): WebviewApi {
  if (typeof acquire === "function") return acquire()
  let mem: unknown
  return { postMessage: () => {}, getState: () => mem, setState: (s) => { mem = s } }
}

export let vscodeApi: WebviewApi = resolve()

/** Called by the browser entry before importing the shared renderer. */
export function installBrowserApi(api: WebviewApi): void {
  if (typeof acquire === 'function' || browserInstalled) throw new Error('Bridge API already installed')
  if (!api.onHostMessage || !api.onDisconnect) throw new Error('Browser API requires a private receive path')
  browserInstalled = true
  vscodeApi = api
}
