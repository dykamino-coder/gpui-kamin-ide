import { afterEach, describe, expect, it, vi } from 'vitest'
import { webFetch } from './web'

// Synthetic public documentation address. DNS and HTTP stay inside the fixture.
vi.mock('node:dns/promises', () => ({ lookup: async () => [{ address: '203.0.113.11', family: 4 }] }))
const url = 'https://fixture.example/doc'
afterEach(() => vi.unstubAllGlobals())

describe('workspace WebFetch outcomes', () => {
  it.each([
    ['ENOTFOUND', 'DNS'], ['UNABLE_TO_VERIFY_LEAF_SIGNATURE', 'TLS'],
    ['ECONNREFUSED', 'connection'], ['UND_ERR_CONNECT_TIMEOUT', 'timed out'],
  ])('identifies %s from a failed fetch without echoing the peer detail', async (code, category) => {
    const cause = Object.assign(new Error('fixture-private-detail'), { code })
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('fetch failed', { cause })))
    const text = (await webFetch({ url, prompt: 'read' })).content[0]!.text
    expect(text).toContain(code)
    expect(text).toContain(category)
    expect(text).not.toContain('fixture-private-detail')
  })
  it.each([403, 407])('retains a received HTTP %i and excludes the response body', async status => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('fixture-private-body', { status })))
    const text = (await webFetch({ url, prompt: 'read' })).content[0]!.text
    expect(text).toContain('HTTP ' + status)
    expect(text).not.toContain('fixture-private-body')
  })
  it('returns the text from a successful workspace request', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('readable fixture')))
    expect((await webFetch({ url, prompt: 'read' })).content[0]!.text).toContain('readable fixture')
  })
})
