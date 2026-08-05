import { describe, expect, it } from 'vitest'
import { withProjectSyncLock, withUserSyncLock } from './lock'

function deferred(): { promise: Promise<void>; resolve: () => void } {
  let resolve!: () => void
  const promise = new Promise<void>((done) => { resolve = done })
  return { promise, resolve }
}

describe('sync snapshot lock', () => {
  it('serializes user and project uploads for the same token', async () => {
    const gate = deferred()
    const order: string[] = []
    const user = withUserSyncLock('token-a', async () => {
      order.push('user:start')
      await gate.promise
      order.push('user:end')
    })
    await Promise.resolve()
    const project = withProjectSyncLock('token-a', '/project', async () => {
      order.push('project:start')
      order.push('project:end')
    })
    await Promise.resolve()

    expect(order).toEqual(['user:start'])
    gate.resolve()
    await Promise.all([user, project])
    expect(order).toEqual(['user:start', 'user:end', 'project:start', 'project:end'])
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
    const second = withProjectSyncLock('token-b', '/project', async () => {
      order.push('b')
    })
    await second

    expect(order).toEqual(['a:start', 'b'])
    gate.resolve()
    await first
  })
})
