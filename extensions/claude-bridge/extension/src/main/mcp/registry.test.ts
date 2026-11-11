import { describe, expect, it } from 'vitest'
import { listMcpRegistry, toRegistryEntry } from './registry'

const page = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status })

describe('MCP registry listing', () => {
  it('maps a remote server record to a registry entry', () => {
    expect(toRegistryEntry({
      server: {
        name: 'com.example/mcp', title: 'Example', description: 'Demo', version: '1.0.0',
        repository: { url: 'https://github.com/example/mcp', source: 'github' }, websiteUrl: 'https://example.com',
        remotes: [{ type: 'streamable-http', url: 'https://mcp.example.com/mcp' }],
      },
      _meta: {},
    })).toEqual({
      name: 'com.example/mcp', title: 'Example', description: 'Demo',
      repository: 'https://github.com/example/mcp', homepage: 'https://example.com',
      install: { type: 'http', url: 'https://mcp.example.com/mcp' },
    })
  })

  it('maps package records to a stdio command and skips unusable records', () => {
    expect(toRegistryEntry({ server: { name: 'npm-one', packages: [{ registryType: 'npm', identifier: '@scope/pkg', transport: { type: 'stdio' } }] } }))
      .toEqual({ name: 'npm-one', install: { type: 'stdio', command: 'npx', args: ['-y', '@scope/pkg'] } })
    expect(toRegistryEntry({ name: 'py-one', packages: [{ registryType: 'pypi', identifier: 'pkg', runtimeHint: 'uvx' }] }))
      .toEqual({ name: 'py-one', install: { type: 'stdio', command: 'uvx', args: ['pkg'] } })
    expect(toRegistryEntry({ server: { name: 'sse-one', remotes: [{ type: 'sse', url: 'https://x.test/sse' }] } })?.install)
      .toEqual({ type: 'sse', url: 'https://x.test/sse' })
    expect(toRegistryEntry({ server: { name: 'bare', packages: [{ registryType: 'oci', identifier: 'img' }] } })).toEqual({ name: 'bare' })
    expect(toRegistryEntry({ server: { description: 'no name' } })).toBeNull()
    expect(toRegistryEntry('nope')).toBeNull()
  })

  it('asks for the latest commercial entries and follows the cursor', async () => {
    const urls: URL[] = []
    const result = await listMcpRegistry(async input => {
      const url = new URL(String(input))
      urls.push(url)
      return url.searchParams.get('cursor')
        ? page({ servers: [{ server: { name: 'second' } }], metadata: { count: 1 } })
        : page({ servers: [{ server: { name: 'first' } }, { server: {} }], metadata: { count: 2, nextCursor: 'first:1' } })
    })
    expect(result).toEqual({ ok: true, servers: [{ name: 'first' }, { name: 'second' }] })
    expect(urls).toHaveLength(2)
    expect(urls[0].searchParams.get('version')).toBe('latest')
    expect(urls[0].searchParams.get('visibility')).toBe('commercial')
    expect(urls[1].searchParams.get('cursor')).toBe('first:1')
  })

  it('stops on a repeated cursor and reports failures without throwing', async () => {
    let calls = 0
    const looping = await listMcpRegistry(async () => { calls++; return page({ servers: [{ server: { name: `s${calls}` } }], metadata: { nextCursor: 'same' } }) })
    expect(looping.ok).toBe(true)
    expect(calls).toBe(2)
    expect(await listMcpRegistry(async () => page({}, 503))).toEqual({ ok: false, error: 'HTTP 503', servers: [] })
    expect(await listMcpRegistry(async () => { throw new Error('offline') })).toEqual({ ok: false, error: 'offline', servers: [] })
  })
})
