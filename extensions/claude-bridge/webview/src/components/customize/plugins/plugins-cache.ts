// Persisted cache for the Plugins panel. Browsing a marketplace (esp. a big one,
// hundreds of plugins) does per-plugin git clones + FS scans host-side and takes
// many seconds cold. We cache the last resolved list in the webview's persisted
// state (survives app restarts) so the panel paints INSTANTLY from cache, then
// revalidates in the background. Keyed per marketplace + the installed list.
import { storage } from "../../../lib/storage.js"

const PREFIX = "plugins.cache.v1."

export function readPluginsCache<T>(key: string): T | null {
  try {
    const raw = storage.getItem(PREFIX + key)
    return raw ? (JSON.parse(raw) as T) : null
  } catch { return null }
}

export function writePluginsCache(key: string, data: unknown): void {
  try { storage.setItem(PREFIX + key, JSON.stringify(data)) } catch { /* quota — non-fatal */ }
}
