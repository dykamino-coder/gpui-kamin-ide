import { beforeEach, describe, expect, it } from 'vitest'

import type { ConnectionState, TabInfo } from '../../shared/types'
import {
  applyConnectionEvent,
  ConnectionSnapshotRequestGate,
  forgetTabConnection,
  mergeInitialTabSnapshot,
  mergeReconnectTabSnapshot,
  reconcilePromptReadiness,
  reconcileCreatedTab,
  reconcileTabSnapshot,
  resetConnectionReconciliation,
} from './tab-connection-reconcile'

function tab(
  id: string,
  status: TabInfo['status'],
  revision: number,
  sessionId?: string,
  authorityGeneration = 1,
  authoritySequence = 100,
): TabInfo {
  return {
    id,
    cwd: `C:\\${id}`,
    label: id,
    folderName: id,
    createdAt: '2026-08-20T00:00:00.000Z',
    status,
    connectionAuthority: 'authority-a',
    connectionAuthorityGeneration: authorityGeneration,
    connectionAuthoritySequence: authoritySequence,
    connectionRevision: revision,
    sessionId,
  }
}

describe('tab connection reconciliation', () => {
  beforeEach(resetConnectionReconciliation)

  it('retains an event that arrives before its tab snapshot', () => {
    const connected: ConnectionState = {
      status: 'connected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 2, sessionId: 'pty-1',
    }
    expect(applyConnectionEvent([], 'A', connected).tabs).toEqual([])

    const result = reconcileTabSnapshot([tab('A', 'connecting', 1)])
    expect(result[0]).toMatchObject({ status: 'connected', connectionRevision: 2, sessionId: 'pty-1' })
  })

  it('rejects stale list and init snapshots after a newer direct event', () => {
    const current = [tab('A', 'connecting', 1)]
    const live = applyConnectionEvent(current, 'A', {
      status: 'connected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 4, sessionId: 'pty-4',
    }).tabs

    expect(reconcileTabSnapshot([tab('A', 'disconnected', 2)])[0]).toMatchObject({
      status: 'connected',
      connectionRevision: 4,
      sessionId: 'pty-4',
    })
    expect(mergeInitialTabSnapshot(live, [tab('A', 'connecting', 3)])[0]).toMatchObject({
      status: 'connected',
      connectionRevision: 4,
    })
  })

  it('accepts a newer authoritative snapshot', () => {
    applyConnectionEvent([tab('A', 'connecting', 1)], 'A', {
      status: 'connected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 2,
    })
    const result = reconcileTabSnapshot([tab('A', 'disconnected', 3)])
    expect(result[0]).toMatchObject({ status: 'disconnected', connectionRevision: 3 })
  })

  it('preserves a tab created while the initial list request was pending', () => {
    const live = [reconcileCreatedTab(tab('B', 'connected', 5, 'pty-B'))]
    const result = mergeInitialTabSnapshot(live, [tab('A', 'connected', 2, 'pty-A')])
    expect(result.map(item => item.id)).toEqual(['A', 'B'])
  })

  it('uses reconnect snapshots for baseline membership but keeps tabs created while pending', () => {
    const baseline = new Set(['A'])
    const current = [
      tab('A', 'connected', 2, 'pty-A'),
      reconcileCreatedTab(tab('B', 'connected', 1, 'pty-B')),
    ]

    const result = mergeReconnectTabSnapshot(current, [], baseline)

    expect(result.map(item => item.id)).toEqual(['B'])
  })

  it('ignores an older reconnect snapshot response after a newer request begins', async () => {
    const gate = new ConnectionSnapshotRequestGate()
    let resolveFirst!: (value: string) => void
    const firstResponse = new Promise<string>((resolve) => { resolveFirst = resolve })
    const applied: string[] = []

    const first = gate.begin()
    const firstApply = firstResponse.then((value) => {
      if (gate.isCurrent(first)) applied.push(value)
    })
    const second = gate.begin()
    if (gate.isCurrent(second)) applied.push('newer')
    resolveFirst('stale')
    await firstApply

    expect(applied).toEqual(['newer'])
    gate.invalidate(second)
    expect(gate.isCurrent(second)).toBe(false)
  })

  it('does not resurrect or update a tab after its close event', () => {
    forgetTabConnection('A')
    expect(applyConnectionEvent([], 'A', {
      status: 'connected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 9,
    })).toEqual({ tabs: [], accepted: false })
    expect(reconcileTabSnapshot([tab('A', 'connected', 9)])).toEqual([])
  })

  it('orders each tab independently', () => {
    const current = [tab('A', 'connecting', 1), tab('B', 'connecting', 7)]
    const afterA = applyConnectionEvent(current, 'A', {
      status: 'connected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 3, sessionId: 'pty-A',
    }).tabs
    const afterStaleBResult = applyConnectionEvent(afterA, 'B', {
      status: 'disconnected', authority: 'authority-a', authorityGeneration: 1, authoritySequence: 100, revision: 6,
    })
    const afterStaleB = afterStaleBResult.tabs
    expect(afterStaleBResult.accepted).toBe(false)
    expect(afterStaleB.find(item => item.id === 'A')).toMatchObject({ status: 'connected', sessionId: 'pty-A' })
    expect(afterStaleB.find(item => item.id === 'B')).toMatchObject({ status: 'connecting', connectionRevision: 7 })
  })

  it('accepts a new authority with a reset revision and retires the old one', () => {
    const current = [tab('A', 'connected', 100, 'pty-old')]
    const nextAuthority = applyConnectionEvent(current, 'A', {
      status: 'connecting',
      authority: 'authority-b',
      authorityGeneration: 1,
      authoritySequence: 200,
      revision: 1,
    })
    expect(nextAuthority).toMatchObject({ accepted: true })
    expect(nextAuthority.tabs[0]).toMatchObject({
      status: 'connecting',
      connectionAuthority: 'authority-b',
      connectionRevision: 1,
    })

    const lateOld = applyConnectionEvent(nextAuthority.tabs, 'A', {
      status: 'disconnected',
      authority: 'authority-a',
      authorityGeneration: 1,
      authoritySequence: 100,
      revision: 101,
    })
    expect(lateOld.accepted).toBe(false)
    expect(lateOld.tabs[0]).toMatchObject({ connectionAuthority: 'authority-b', status: 'connecting' })
  })

  it('rejects an unseen older authority after a newer live event', () => {
    const current = [tab('A', 'connected', 10, 'pty-old', 1, 100)]
    const currentManager = applyConnectionEvent(current, 'A', {
      status: 'connected', authority: 'authority-b', authorityGeneration: 3, authoritySequence: 1, revision: 1, sessionId: 'pty-new',
    }).tabs

    const staleSnapshot = reconcileTabSnapshot([{
      ...tab('A', 'disconnected', 50, undefined, 2, 999),
      connectionAuthority: 'authority-c',
    }])
    expect(staleSnapshot[0]).toMatchObject({
      status: 'connected', connectionAuthority: 'authority-b', connectionAuthorityGeneration: 3, sessionId: 'pty-new',
    })
    expect(currentManager[0]).toMatchObject({ connectionAuthority: 'authority-b' })
  })

  it('rehydrates prompt readiness only from terminal connection states', () => {
    const current = new Map<string, boolean>([['A', false], ['B', true], ['C', true]])
    const result = reconcilePromptReadiness(current, [
      { id: 'A', status: 'connected' },
      { id: 'B', status: 'connecting' },
      { id: 'C', status: 'error' },
    ])

    expect(result).not.toBe(current)
    expect([...result]).toEqual([['A', true], ['B', true], ['C', false]])
    expect(reconcilePromptReadiness(result, [{ id: 'A', status: 'connected' }])).toBe(result)
  })
})
