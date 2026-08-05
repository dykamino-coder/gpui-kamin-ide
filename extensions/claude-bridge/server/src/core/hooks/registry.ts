// In-memory registry of registered hooks per session. Each entry maps a
// uuid (carried in the proxy URL `…/hooks/:sessionId/:event/:hookId`) to
// the original user-supplied handler config so the bridge can route an
// incoming webhook back to the correct hook implementation.

import { randomUUID } from 'crypto'
import type { HookEvent, HookHandler, HookMatcher, HookSettings, HookSource, RegisteredHook } from './types'
import { resolveHost } from './host-resolver'

/** Map<sessionId, Map<hookId, RegisteredHook>>. */
const bySession: Map<string, Map<string, RegisteredHook>> = new Map()

export function clearSession(sessionId: string): void {
  bySession.delete(sessionId)
}

export function getHook(sessionId: string, hookId: string): RegisteredHook | undefined {
  return bySession.get(sessionId)?.get(hookId)
}

export function listSession(sessionId: string): RegisteredHook[] {
  return Array.from(bySession.get(sessionId)?.values() ?? [])
}

/**
 * Register every hook in a HookSettings JSON. Returns the same object
 * with each handler annotated by `__hookId` for the rewriter to use.
 */
export function registerSessionHooks(
  sessionId: string,
  hooks: HookSettings,
  sourceFn: HookSource | ((m: HookMatcher) => HookSource),
): Map<HookHandler, RegisteredHook> {
  let bucket = bySession.get(sessionId)
  if (!bucket) {
    bucket = new Map()
    bySession.set(sessionId, bucket)
  }
  const annotated = new Map<HookHandler, RegisteredHook>()
  const resolveSource = (m: HookMatcher): HookSource =>
    typeof sourceFn === 'function' ? sourceFn(m) : sourceFn

  for (const [event, matchers] of Object.entries(hooks)) {
    if (!Array.isArray(matchers)) continue
    for (const m of matchers) {
      if (!m || !Array.isArray(m.hooks)) continue
      const source = resolveSource(m)
      for (const h of m.hooks) {
        const id = randomUUID()
        // Plugin commands reference files and binaries on the client host.
        // Claude plugins do not know the bridge-only `host` extension, so their
        // default must be local even for events otherwise preferred on server.
        const explicitHost = h.type === 'command' ? h.host : undefined
        const effectiveHost = source.kind === 'plugin' && h.type === 'command' && (!explicitHost || explicitHost === 'auto')
          ? 'local'
          : resolveHost(event as HookEvent, h, m.matcher)
        const reg: RegisteredHook = {
          id,
          sessionId,
          event: event as HookEvent,
          matcher: m.matcher,
          handler: h,
          source,
          effectiveHost,
        }
        bucket.set(id, reg)
        annotated.set(h, reg)
      }
    }
  }
  return annotated
}
