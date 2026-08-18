import { describe, expect, it } from 'vitest'
import { withSyncAuthorization } from './request-auth'

describe('withSyncAuthorization', () => {
  it('injects the host token into sync requests', () => {
    expect(withSyncAuthorization('/api/sync/abc/tree', {
      headers: { Accept: 'application/json', Authorization: 'Bearer stale' },
    }, 'owner-secret')).toEqual({
      headers: { Accept: 'application/json', Authorization: 'Bearer owner-secret' },
    })
  })

  it('does not attach the bridge token to unrelated dashboard routes', () => {
    const init = { method: 'POST', body: '{}' }
    expect(withSyncAuthorization('/api/dashboard/tokens/resolve', init, 'owner-secret')).toBe(init)
  })
})
