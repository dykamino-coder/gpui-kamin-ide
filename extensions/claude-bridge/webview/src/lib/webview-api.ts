// The single `acquireVsCodeApi()` handle for the whole webview. VS Code (and
// KaminIDE) allow it to be called EXACTLY ONCE per webview — everything that
// needs postMessage or persisted state imports it from here. In a non-webview
// context (e.g. a unit test) it falls back to an in-memory stub.
interface VsCodeApi {
  postMessage: (msg: unknown) => void
  getState: () => unknown
  setState: (state: unknown) => void
}

function resolve(): VsCodeApi {
  const acquire = (globalThis as { acquireVsCodeApi?: () => VsCodeApi }).acquireVsCodeApi
  if (typeof acquire === "function") return acquire()
  let mem: unknown
  return { postMessage: () => {}, getState: () => mem, setState: (s) => { mem = s } }
}

export const vscodeApi: VsCodeApi = resolve()
