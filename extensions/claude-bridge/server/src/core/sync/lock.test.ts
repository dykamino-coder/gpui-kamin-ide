import { describe, expect, it } from 'vitest'
import { withProjectSyncLock, withSyncSnapshotLock, withUserSyncLock } from './lock'

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

describe('sync snapshot lock', () => {
  it('serializes user upload, project upload and session snapshot for one token', async () => {
    const gate = deferred()
    const order: string[] = []
    const user = withUserSyncLock('token-a', async () => {
      order.push('user:start')
      await gate.promise
      order.push('user:end')
    })
    await Promise.resolve()
    const project = withProjectSyncLock('token-a', '/project', async () => {
      order.push('project')
    })
    const snapshot = withSyncSnapshotLock('token-a', () => {
      order.push('snapshot')
    })
    await Promise.resolve()

    expect(order).toEqual(['user:start'])
    gate.resolve()
    await Promise.all([user, project, snapshot])
    expect(order).toEqual(['user:start', 'user:end', 'project', 'snapshot'])
  })

  it('does not serialize independent tokens', async () => {
    const gate = deferred()
    const order: string[] = []
    const first = withUserSyncLock('token-a', async () => {
      order.push('a:start')
      await gate.promise
      order.push('a:end')
    })
    await Promise.resolve()
    await withSyncSnapshotLock('token-b', () => {
      order.push('b')
    })

    expect(order).toEqual(['a:start', 'b'])
    gate.resolve()
    await first
  })
})
