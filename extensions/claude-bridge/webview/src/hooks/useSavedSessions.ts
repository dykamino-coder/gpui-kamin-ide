import { useEffect, useCallback, useState } from 'preact/hooks'
import type { SavedSession } from '../../shared/types'
import { savedSessionsSignal } from '../signals/saved-sessions'
import { serverFetch } from '../lib/server-fetch'

/** Mirror a sidebar session removal to the server: soft-delete the
 *  session_tokens row + unlink the JSONL on disk so the bridge dashboard
 *  Stats grid drops it from every aggregate (matches client sidebar). Routed
 *  through the ext-host (Node) — the webview CSP blocks direct http. */
async function softDeleteServerSession(bridge: any, conversationId: string): Promise<void> {
  try {
    const cfg = await bridge.getConfig?.()
    if (!cfg?.serverUrl) return
    const httpUrl = String(cfg.serverUrl).replace(/^ws/, 'http').replace(/\/ws\/session$/, '')
    await serverFetch(bridge, httpUrl, `/api/dashboard/sessions/${encodeURIComponent(conversationId)}?rmFile=1`, {
      method: 'DELETE',
    })
  } catch { /* ignore — server is best-effort here */ }
}

interface UseSavedSessionsResult {
  savedSessions: SavedSession[]
  loaded: boolean
  reload: () => Promise<void>
  removeSaved: (conversationId: string) => void
  removeSavedBatch: (conversationIds: string[]) => void
}

/** Saved-session list backed by a global signal so other components
 *  (TabsBar in particular — pinned ghost tabs need a human title to
 *  display when their own `g.title` is empty) can read the same data
 *  without round-tripping through bridge.getSavedSessions on each
 *  render. The hook still owns load orchestration and remove
 *  operations; consumers outside this hook just read
 *  `savedSessionsSignal` / `savedSessionsByConvId`. */
export function useSavedSessions(): UseSavedSessionsResult {
  const [loaded, setLoaded] = useState(false)
  const savedSessions = savedSessionsSignal.value

  const load = useCallback(async () => {
    const bridge = (window as any).electronBridge
    if (!bridge?.getSavedSessions) return
    try {
      const sessions = await bridge.getSavedSessions()
      savedSessionsSignal.value = sessions
      setLoaded(true)
    } catch {
      // ignore
    }
  }, [])

  const removeSaved = useCallback((conversationId: string) => {
    const bridge = (window as any).electronBridge
    if (!bridge) return
    // Optimistic update — drop from the signal immediately so the UI
    // reacts before the IPC round-trip completes.
    savedSessionsSignal.value = savedSessionsSignal.value.filter(
      s => s.conversationId !== conversationId,
    )
    bridge.removeSavedSession(conversationId)
    bridge.deleteSessionData?.(conversationId)
    // Mirror the deletion to the server's session_tokens (soft-delete +
    // unlink the JSONL file) so its Stats grid, Sessions counter, and
    // per-token sessions list stay in lockstep with the client sidebar.
    void softDeleteServerSession(bridge, conversationId)
  }, [])

  const removeSavedBatch = useCallback((conversationIds: string[]) => {
    const bridge = (window as any).electronBridge
    if (!bridge) return
    const set = new Set(conversationIds)
    savedSessionsSignal.value = savedSessionsSignal.value.filter(
      s => !set.has(s.conversationId),
    )
    for (const id of conversationIds) {
      bridge.removeSavedSession(id)
      bridge.deleteSessionData?.(id)
      void softDeleteServerSession(bridge, id)
    }
  }, [])

  useEffect(() => { load() }, [load])

  // Subscribe to the main-process broadcast so closing a tab (or any other
  // savedSession mutation) refreshes the sidebar immediately, without
  // waiting for the next mount.
  useEffect(() => {
    const bridge = (window as any).electronBridge
    const off = bridge?.onSavedSessionsChanged?.(() => { void load() })
    return () => { if (typeof off === 'function') off() }
  }, [load])

  return { savedSessions, loaded, reload: load, removeSaved, removeSavedBatch }
}
