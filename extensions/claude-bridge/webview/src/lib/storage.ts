// Small UI settings use the host state API. In native CEF that API is
// document-local; callers must not infer durable app persistence from it.
// Console snapshots have their own bounded cache and never enter state IPC.
import { vscodeApi } from "./webview-api.js"

const STATE_KEY = "ls"

function load(): Record<string, string> {
  const s = vscodeApi.getState()
  const ls = (s as { [STATE_KEY]?: unknown } | null)?.[STATE_KEY]
  return ls && typeof ls === "object"
    ? Object.fromEntries(Object.entries(ls as Record<string, string>).filter(([key]) => !key.startsWith("xterm-snapshot:")))
    : {}
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
