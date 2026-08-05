// Ghost-tab resume flow extracted from TabsBar.tsx (Sprint 5 / Stage E1).
// A ghost is a pinned session whose live PTY tab was closed; we kept the
// chip visible. Clicking it asks the bridge to resume the conversation by
// id, falling back to a fresh tab if the conversationId is missing.

import type { KaminBridgeApi, PersistedTabState, SavedSession } from '../../../shared/types'
import { showToast } from '../../signals/toasts'

export async function resumeGhostTab(
  bridge: KaminBridgeApi,
  ghost: PersistedTabState,
  savedByConv: Map<string, SavedSession>,
): Promise<void> {
  // Resolve the cleanest title we have for this ghost. `g.title` may be the
  // auto counter ("#1") if the original session never got a topic; fall back
  // to the saved-sessions store entry.
  const saved = ghost.conversationId ? savedByConv.get(ghost.conversationId) : undefined
  const cleanGhostTitle = (ghost.title?.trim() && !/^#\d+$/.test(ghost.title.trim())) ? ghost.title.trim() : ''
  const fallbackSaved = saved?.label?.trim() && !/^#\d+$/.test(saved.label.trim()) ? saved.label.trim() : ''
  const resolvedSessionLabel = cleanGhostTitle || fallbackSaved || ''
  const title = resolvedSessionLabel || ghost.cwd.split(/[\\/]/).filter(Boolean).pop() || 'session'

  try {
    const cfg: any = await bridge.getConfig?.()
    if (!cfg?.serverUrl || !cfg?.token) {
      showToast({ type: 'error', title: 'Cannot resume', message: 'No server/token configured — open Settings first.' })
      return
    }
    // Warn if the original cwd no longer exists. Resume still goes through
    // (CLI warns / falls back) but the user knows files won't match. Skip
    // the toast for no-folder sessions — there's no real "working
    // directory" to be missing.
    try {
      const cwd = ghost.cwd?.trim()
      const isNoFolderSession = !cwd || cwd === '.' || cwd === '/'
      if (!isNoFolderSession) {
        const exists = await bridge.pathExists?.(cwd)
        if (exists === false) {
          showToast({
            type: 'info',
            title: `⚠ ${title}`,
            message: `Working directory ${cwd} no longer exists — resuming anyway.`,
          })
        }
      }
    } catch { /* pathExists is optional */ }

    if (ghost.conversationId) {
      await bridge.resumeSession?.({
        ...cfg,
        cwd: ghost.cwd,
        conversationId: ghost.conversationId,
        sessionLabel: resolvedSessionLabel || undefined,
      })
    } else {
      await bridge.createTab?.({ ...cfg, cwd: ghost.cwd })
    }
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    showToast({
      type: 'error',
      title: `Failed to resume "${title}"`,
      message: msg || 'Unknown error — right-click pin to remove this chip.',
    })
  }
}
