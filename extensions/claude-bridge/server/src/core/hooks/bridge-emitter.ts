// Fires bridge-side custom events that have no Claude-CLI counterpart.
// User can register hooks for these the same way as for the 27 CLI events;
// matcher/host/handler semantics are identical.
//
// Usage from anywhere in the bridge:
//   await emitBridgeEvent('BridgeReconnect', { sessionId, downtimeSec: 12 })
// Bridge dispatches to every registered hook with a matching event name.

import type { BridgeHookEvent, HookInputPayload } from './types'
import { listSession } from './registry'
import { dispatchHook } from './dispatcher'
import { sessions } from '../pty/session-core'
import { debugLog } from '../logging'

/**
 * Fire a bridge-emit event for one specific session, or for all sessions
 * (when the event is global, e.g. AutoUpdateAvailable / MarketplaceUpdated).
 */
export async function emitBridgeEvent(
  event: BridgeHookEvent,
  payload: Record<string, unknown> & { sessionId?: string },
): Promise<void> {
  const targetSessions = payload.sessionId
    ? [payload.sessionId]
    : Array.from(sessions.keys())

  for (const sessionId of targetSessions) {
    const sessionHooks = listSession(sessionId).filter(h => h.event === event)
    if (sessionHooks.length === 0) continue

    const hookPayload: HookInputPayload = {
      hook_event_name: event,
      session_id: sessionId,
      ...payload,
    }

    const session = sessions.get(sessionId)
    const ws = session?.ws ?? null

    for (const reg of sessionHooks) {
      // Optional: filter by matcher if hook has one (e.g. specific connector
      // name in ConnectorStatusChanged). Naive substring match for now.
      if (reg.matcher) {
        const targetField = (payload as Record<string, unknown>)[reg.matcher]
        if (targetField === undefined) continue
      }
      try {
        await dispatchHook(sessionId, reg.id, hookPayload, ws)
      } catch (err) {
        debugLog('Bridge-emit hook error', { event, hookId: reg.id, err })
      }
    }
  }
}
