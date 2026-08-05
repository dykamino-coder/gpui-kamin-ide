import { describe, expect, it } from 'vitest'
import { collectMcpList } from './pagination'

describe('collectMcpList', () => {
  it('collects every page and forwards only the negotiated cursor', async () => {
    const calls: Array<{ method: string; params: Record<string, unknown> }> = []
    const pages = [
      { tools: [{ name: 'one' }], nextCursor: 'page-2' },
      { tools: [{ name: 'two' }] },
    ]

    const tools = await collectMcpList<{ name: string }>(async (method, params) => {
      calls.push({ method, params })
      return pages[calls.length - 1]
    }, 'tools/list', 'tools')

    expect(tools.map(tool => tool.name)).toEqual(['one', 'two'])
    expect(calls).toEqual([
      { method: 'tools/list', params: {} },
      { method: 'tools/list', params: { cursor: 'page-2' } },
    ])
  })

  it('rejects cursor cycles instead of looping forever', async () => {
    let page = 0
    await expect(collectMcpList(async () => {
      page++
      return page === 1
        ? { resources: [], nextCursor: 'same' }
        : { resources: [], nextCursor: 'same' }
    }, 'resources/list', 'resources')).rejects.toThrow('repeated cursor')
  })

  it('rejects catalogs that exceed page or item bounds', async () => {
    let page = 0
    await expect(collectMcpList(async () => ({ prompts: [], nextCursor: `page-${++page}` }), 'prompts/list', 'prompts', {
      maxPages: 2,
    })).rejects.toThrow('2-page pagination limit')

    await expect(collectMcpList(async () => ({ resourceTemplates: [1, 2, 3] }), 'resources/templates/list', 'resourceTemplates', {
      maxItems: 2,
    })).rejects.toThrow('2-item catalog limit')
  })

  it('rejects malformed pagination metadata', async () => {
    await expect(collectMcpList(async () => ({ tools: {}, nextCursor: 42 }), 'tools/list', 'tools'))
      .rejects.toThrow('non-array tools')
    await expect(collectMcpList(async () => ({ tools: [], nextCursor: 42 }), 'tools/list', 'tools'))
      .rejects.toThrow('non-string nextCursor')
  })
})
