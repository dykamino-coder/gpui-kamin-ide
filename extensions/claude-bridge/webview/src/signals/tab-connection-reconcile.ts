import type { ConnectionState, TabInfo } from '../../shared/types'

/** Latest connection event per tab, including events that arrive before the
 * initial listTabs response has materialised that tab in the renderer. */
const latest = new Map<string, ConnectionState>()
/** Authorities superseded by a newer manager for this tab. A late frame from a
 * dead extension host must not take authority back after respawn. */
const retiredAuthorities = new Map<string, Set<string>>()
const closed = new Set<string>()

/** Orders asynchronous host snapshots within one mounted renderer. A response
 * from an older reconnect must not apply after a later reconnect already began. */
export class ConnectionSnapshotRequestGate {
  private current = 0

  begin(): number {
    this.current += 1
    return this.current
  }

  isCurrent(request: number): boolean {
    return request === this.current
  }

  invalidate(request: number): void {
    if (request === this.current) this.current += 1
  }
}

function revision(state: ConnectionState): number {
  return state.revision
}

function fromTab(tab: TabInfo): ConnectionState {
  return {
    status: tab.status,
    authority: tab.connectionAuthority,
    authorityGeneration: tab.connectionAuthorityGeneration,
    authoritySequence: tab.connectionAuthoritySequence,
    revision: tab.connectionRevision,
    sessionId: tab.sessionId,
    error: tab.error,
    nextRetryAt: tab.nextRetryAt,
    retryAttempt: tab.retryAttempt,
  }
}

function withState(tab: TabInfo, state: ConnectionState): TabInfo {
  return {
    ...tab,
    status: state.status,
    connectionAuthority: state.authority,
    connectionAuthorityGeneration: state.authorityGeneration,
    connectionAuthoritySequence: state.authoritySequence,
    connectionRevision: revision(state),
    sessionId: state.sessionId,
    error: state.error,
    nextRetryAt: state.nextRetryAt,
    retryAttempt: state.retryAttempt,
  }
}

function sameState(tab: TabInfo, state: ConnectionState): boolean {
  return tab.status === state.status
    && tab.connectionAuthority === state.authority
    && tab.connectionAuthorityGeneration === state.authorityGeneration
    && tab.connectionAuthoritySequence === state.authoritySequence
    && tab.connectionRevision === revision(state)
    && tab.sessionId === state.sessionId
    && tab.error === state.error
    && tab.nextRetryAt === state.nextRetryAt
    && tab.retryAttempt === state.retryAttempt
}

function remember(tabId: string, candidate: ConnectionState): { state: ConnectionState; accepted: boolean } {
  const remembered = latest.get(tabId)
  if (!remembered) {
    latest.set(tabId, candidate)
    return { state: candidate, accepted: true }
  }
  if (remembered.authority === candidate.authority) {
    if (revision(candidate) < revision(remembered)) return { state: remembered, accepted: false }
    latest.set(tabId, candidate)
    return { state: candidate, accepted: true }
  }

  // UUIDs establish identity, not ordering. The parent-host generation survives
  // extension-host respawns; the child-local sequence orders managers within a
  // generation. Compare the tuple so an unseen delayed child can never retake
  // authority, even if the wall clock moves backwards.
  const generationDelta = candidate.authorityGeneration - remembered.authorityGeneration
  const sequenceDelta = candidate.authoritySequence - remembered.authoritySequence
  if (generationDelta < 0 || (generationDelta === 0 && sequenceDelta <= 0)) {
    return { state: remembered, accepted: false }
  }

  const retired = retiredAuthorities.get(tabId) ?? new Set<string>()
  if (retired.has(candidate.authority)) return { state: remembered, accepted: false }
  retired.add(remembered.authority)
  retiredAuthorities.set(tabId, retired)
  latest.set(tabId, candidate)
  return { state: candidate, accepted: true }
}

function reconcileTab(tab: TabInfo): TabInfo {
  const snapshot = fromTab(tab)
  const result = remember(tab.id, snapshot)
  return result.accepted ? tab : withState(tab, result.state)
}

/** Apply a live connection event. Missing tabs are intentional: remember the
 * state and apply it when tab:created/listTabs arrives instead of dropping it. */
export interface ConnectionEventResult {
  tabs: TabInfo[]
  accepted: boolean
}

export function applyConnectionEvent(tabList: TabInfo[], tabId: string, state: ConnectionState): ConnectionEventResult {
  if (closed.has(tabId)) return { tabs: tabList, accepted: false }
  const currentTab = tabList.find(tab => tab.id === tabId)
  const currentState = currentTab ? fromTab(currentTab) : undefined
  if (currentState) remember(tabId, currentState)
  const result = remember(tabId, state)
  if (!result.accepted) return { tabs: tabList, accepted: false }
  let changed = false
  const next = tabList.map((tab) => {
    if (tab.id !== tabId || sameState(tab, state)) return tab
    changed = true
    return withState(tab, state)
  })
  return { tabs: changed ? next : tabList, accepted: true }
}

/** Reconcile a host tab-list snapshot with newer direct connection events. */
export function reconcileTabSnapshot(snapshot: TabInfo[]): TabInfo[] {
  return snapshot.filter(tab => !closed.has(tab.id)).map(reconcileTab)
}

/** Keep the composer's secondary prompt-readiness gate aligned with an
 * authoritative connection snapshot. `connecting` remains input-blocked by
 * the connection status itself, so it intentionally preserves the last CLI
 * readiness signal until authentication succeeds or the socket terminates. */
export function reconcilePromptReadiness(
  current: Map<string, boolean>,
  connectionStates: ReadonlyArray<Pick<TabInfo, 'id' | 'status'>>,
): Map<string, boolean> {
  let next: Map<string, boolean> | undefined
  for (const tab of connectionStates) {
    const ready = tab.status === 'connected'
      ? true
      : tab.status === 'disconnected' || tab.status === 'error'
        ? false
        : undefined
    if (ready === undefined || current.get(tab.id) === ready) continue
    if (!next) next = new Map(current)
    next.set(tab.id, ready)
  }
  return next ?? current
}

/** Initial listTabs may have been computed before tab/connection events that
 * already reached this renderer. Preserve such live rows instead of replacing
 * them with an older async response. */
export function mergeInitialTabSnapshot(current: TabInfo[], snapshot: TabInfo[]): TabInfo[] {
  const incoming = reconcileTabSnapshot(snapshot)
  const seen = new Set(incoming.map(tab => tab.id))
  return [...incoming, ...current.filter(tab => !seen.has(tab.id) && !closed.has(tab.id)).map(reconcileTab)]
}

/** Reconnect snapshots are authoritative for membership that existed when the
 * request began: an absent baseline tab may have been closed inside the event
 * gap. Preserve only rows created after the request, while connection slices
 * still reconcile through authority/revision ordering. */
export function mergeReconnectTabSnapshot(
  current: TabInfo[],
  snapshot: TabInfo[],
  baselineIds: ReadonlySet<string>,
): TabInfo[] {
  const incoming = reconcileTabSnapshot(snapshot)
  const seen = new Set(incoming.map(tab => tab.id))
  const createdWhilePending = current.filter(tab => (
    !baselineIds.has(tab.id) && !seen.has(tab.id) && !closed.has(tab.id)
  )).map(reconcileTab)
  return [...incoming, ...createdWhilePending]
}

export function reconcileCreatedTab(tab: TabInfo): TabInfo {
  closed.delete(tab.id)
  retiredAuthorities.delete(tab.id)
  return reconcileTab(tab)
}

export function forgetTabConnection(tabId: string): void {
  latest.delete(tabId)
  retiredAuthorities.delete(tabId)
  closed.add(tabId)
}

/** Test-only reset for module-scoped ordering state. */
export function resetConnectionReconciliation(): void {
  latest.clear()
  retiredAuthorities.clear()
  closed.clear()
}
