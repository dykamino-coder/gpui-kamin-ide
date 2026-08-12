import 'reflect-metadata'
import { describe, expect, it, vi } from 'vitest'

vi.mock('../auth/tokens', () => ({
  resolveToken: vi.fn(async (token: string) => token === 'owner-secret'
    ? { tokenId: 'owner-id', userName: 'owner' }
    : null),
}))

import { createSyncRoutes, getUserSyncDir, tokenHash } from './routes'

describe('sync route isolation', () => {
  it('rejects decoded path traversal before resolving a filesystem path', async () => {
    expect(() => getUserSyncDir('../../tmp')).toThrow('Invalid tokenId')
    const app = createSyncRoutes()
    const response = await app.request('/api/sync/..%2F..%2Ftmp/status', {
      headers: { Authorization: 'Bearer owner-secret' },
    })
    expect(response.status).toBe(400)
  })

  it('requires a valid bearer that owns the hash in the URL', async () => {
    const app = createSyncRoutes()
    const hash = tokenHash('owner-secret')
    const unauthenticated = await app.request(`/api/sync/${hash}/status`)
    expect(unauthenticated.status).toBe(401)

    const wrongOwner = await app.request(`/api/sync/${hash}/status`, {
      headers: { Authorization: 'Bearer someone-else' },
    })
    expect(wrongOwner.status).toBe(401)

    const otherHash = tokenHash('different-secret')
    const mismatched = await app.request(`/api/sync/${otherHash}/status`, {
      headers: { Authorization: 'Bearer owner-secret' },
    })
    expect(mismatched.status).toBe(403)
  })

  it('rejects an oversized upload before parsing its JSON', async () => {
    const app = createSyncRoutes()
    const hash = tokenHash('owner-secret')
    const response = await app.request(`/api/sync/${hash}/user`, {
      method: 'POST',
      headers: {
        Authorization: 'Bearer owner-secret',
        'Content-Type': 'application/json',
        'Content-Length': String(11 * 1024 * 1024),
      },
      body: '{}',
    })
    expect(response.status).toBe(413)
  })
})
