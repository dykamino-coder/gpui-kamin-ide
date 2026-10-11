import crypto from 'node:crypto'

// Process-local correlation only. Never persist this key or emit input strings.
const key = crypto.randomBytes(32)
const boot = crypto.randomBytes(8).toString('hex')
const MAX_EVENTS = 256
const events: SkillsReloadEvent[] = []

export interface SkillsReloadContext {
  token: string
  scope: string
  source: 'user' | 'project'
  snapshotRevision: string
  changed: boolean
  count: number
}
export type ReloadReason =
  | 'sync-ready'
  | 'prompt-ready'
  | 'reattach'
  | 'raw-input-cleared'
  | 'queue-drained'
  | 'teardown'
export interface SkillsReloadEvent extends SkillsReloadContext {
  boot: string
  at: number
  stage: 'snapshot' | 'queued' | 'started' | 'enter-written' | 'cancelled'
  session?: string
  maintenanceRevision?: number
  queuedAt?: number
  reason?: ReloadReason
  blockedBy?:
    | 'not-running'
    | 'detached'
    | 'prompt-not-ready'
    | 'raw-input'
    | 'submission-active'
    | 'user-queue'
    | 'input-quiet'
    | 'ready'
  coalesced?: boolean
  overlayRefreshed?: boolean
}

function pseudonym(domain: string, ...values: string[]): string {
  const hash = crypto.createHmac('sha256', key).update(domain)
  // Length-delimit every component; different path/content splits must not alias.
  for (const value of values) hash.update(JSON.stringify(value))
  return hash.digest('hex').slice(0, 32)
}

function append(event: SkillsReloadEvent): void {
  events.push(event)
  if (events.length > MAX_EVENTS) events.shift()
}

export function recordSkillsSnapshot(
  tokenId: string,
  source: 'user' | 'project',
  projectPath: string | undefined,
  snapshot: Record<string, string>,
  changed: boolean,
): SkillsReloadContext {
  const token = pseudonym('token', tokenId)
  const scope = pseudonym('scope', token, source, projectPath ?? '')
  const hash = crypto.createHmac('sha256', key).update('snapshot').update(scope)
  for (const name of Object.keys(snapshot).sort()) {
    hash.update(JSON.stringify([name, snapshot[name]]))
  }
  const context: SkillsReloadContext = {
    token,
    scope,
    source,
    snapshotRevision: hash.digest('hex').slice(0, 32),
    changed,
    count: Object.keys(snapshot).length,
  }
  append({ ...context, boot, at: Date.now(), stage: 'snapshot' })
  return context
}

export function recordSkillsMaintenance(
  context: SkillsReloadContext | undefined,
  sessionId: string,
  stage: Exclude<SkillsReloadEvent['stage'], 'snapshot'>,
  maintenanceRevision: number,
  queuedAt: number,
  details: Pick<SkillsReloadEvent, 'reason' | 'blockedBy' | 'coalesced' | 'overlayRefreshed'> = {},
): void {
  if (!context) return
  // Explicit allowlist, rather than spreading arbitrary sync/PTY objects.
  append({
    token: context.token,
    scope: context.scope,
    source: context.source,
    snapshotRevision: context.snapshotRevision,
    changed: context.changed,
    count: context.count,
    boot,
    at: Date.now(),
    stage,
    session: pseudonym('session', context.token, sessionId),
    maintenanceRevision,
    queuedAt,
    reason: details.reason,
    blockedBy: details.blockedBy,
    coalesced: details.coalesced,
    overlayRefreshed: details.overlayRefreshed,
  })
}

/** Owner-scoped diagnostic window. Eviction/restart deliberately lose history. */
export function readSkillsReloadTelemetry(tokenId: string): SkillsReloadEvent[] {
  const token = pseudonym('token', tokenId)
  return events.filter((event) => event.token === token).map((event) => ({ ...event }))
}
