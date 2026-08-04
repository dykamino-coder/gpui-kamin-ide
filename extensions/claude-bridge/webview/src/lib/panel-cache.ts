// Generic persisted cache for Customize panels (Skills / Agents / Hooks / Stats).
// Every panel refetches its list on mount, and those fetches hit the host over
// the bridge (FS scans, git-backed skill/agent enumeration, dashboard HTTP for
// Stats) — a cold open shows a bare "Loading…" for a beat every single time.
// We stash the last resolved payload in the webview's persisted state (survives
// app restarts, same primitive the Plugins panel uses) so a panel paints
// INSTANTLY from cache, then revalidates in the background.
//
// Keep this deliberately dumb: JSON in, JSON out, keyed by a caller-supplied
// string. Bump PREFIX to invalidate every cached panel at once after a shape
// change.
import { storage } from './storage.js'

const PREFIX = 'panel.cache.v1.'

export function readPanelCache<T>(key: string): T | null {
  try {
    const raw = storage.getItem(PREFIX + key)
    return raw ? (JSON.parse(raw) as T) : null
  } catch { return null }
}

export function writePanelCache(key: string, data: unknown): void {
  try { storage.setItem(PREFIX + key, JSON.stringify(data)) } catch { /* quota — non-fatal */ }
}
