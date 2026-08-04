// Persisted key→value store for the webview UI (tab layout, theme, panel sizes).
// The Electron renderer used window.localStorage, but a sandboxed webview iframe
// (opaque origin) forbids it — VS Code webviews persist via the state API
// instead. This is the CORRECT replacement (not a global localStorage monkey-
// patch): a Storage-shaped module backed by vscodeApi.getState/setState, so the
// copied renderer's `storage.getItem/setItem` calls persist properly + portably
// (works the same in real VS Code).
import { vscodeApi } from "./webview-api.js"

const STATE_KEY = "ls"

function load(): Record<string, string> {
  const s = vscodeApi.getState()
  const ls = (s as { [STATE_KEY]?: unknown } | null)?.[STATE_KEY]
  return ls && typeof ls === "object" ? { ...(ls as Record<string, string>) } : {}
}

let mem = load()

function persist(): void {
  const s = (vscodeApi.getState() as Record<string, unknown> | null) ?? {}
  vscodeApi.setState({ ...s, [STATE_KEY]: mem })
}

export const storage = {
  getItem(key: string): string | null {
    return Object.prototype.hasOwnProperty.call(mem, key) ? mem[key] : null
  },
  setItem(key: string, value: string): void {
    mem[key] = String(value)
    persist()
  },
  removeItem(key: string): void {
    if (key in mem) { delete mem[key]; persist() }
  },
  clear(): void {
    mem = {}
    persist()
  },
}
