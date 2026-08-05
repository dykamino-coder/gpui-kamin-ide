// Tiny helper used by client-host code to fire a bridge-emit hook
// event. Posts to bridge's /api/hooks/emit/:event endpoint with the active
// session's bearer token. Fire-and-forget by design — never blocks the
// caller, errors are swallowed (a missing emit should never crash the app).

import type { ConfigStore } from '../config/store'

let configStoreRef: ConfigStore | null = null

/** Wired once at app start so emit() can read the current bearer/url
 *  without a giant chain of constructor params from every call site. */
export function setHookEmitConfigStore(cs: ConfigStore): void {
  configStoreRef = cs
}

export function emitBridgeHookEvent(event: string, payload: Record<string, unknown> = {}): void {
  const cfg = configStoreRef?.get()
  if (!cfg?.serverUrl || !cfg?.token) return
  const httpBase = cfg.serverUrl.replace(/^wss?:\/\//, m => m === 'wss://' ? 'https://' : 'http://').replace(/\/ws.*$/, '')
  const url = `${httpBase}/api/hooks/emit/${encodeURIComponent(event)}`
  // Fire-and-forget; ignore both fetch and parse errors.
  fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${cfg.token}`,
    },
    body: JSON.stringify(payload),
  }).catch(() => { /* swallow */ })
}
