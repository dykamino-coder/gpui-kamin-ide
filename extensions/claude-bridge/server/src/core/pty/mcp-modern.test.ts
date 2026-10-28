import { describe, expect, it } from 'vitest'
import { MODERN_MCP, TASK_EXTENSION, modernRequest, supportsTasks } from './mcp-modern'
const meta = {
  'io.modelcontextprotocol/protocolVersion': MODERN_MCP,
  'io.modelcontextprotocol/clientCapabilities': { extensions: { [TASK_EXTENSION]: {} } },
}
function negotiate(method: string, params: unknown, headers: Record<string, string | undefined> = {}) {
  return modernRequest(
    { method, params },
    (key) => ({ 'MCP-Protocol-Version': MODERN_MCP, 'Mcp-Method': method, ...headers })[key],
  )
}
describe('MCP per-request negotiation', () => {
  it('keeps legacy handshake and calls without modern metadata', () => {
    expect(modernRequest({ method: 'tools/call', params: {} }, () => undefined)).toEqual({ modern: false })
    expect(supportsTasks({})).toBe(false)
  })
  it('requires independent task opt-in on each request', () => {
    expect(negotiate('tools/call', { _meta: meta, name: 'Bash' }, { 'Mcp-Name': 'Bash' })).toEqual({ modern: true })
    expect(supportsTasks({ _meta: meta })).toBe(true)
    expect(supportsTasks({ _meta: { ...meta, 'io.modelcontextprotocol/clientCapabilities': {} } })).toBe(false)
  })
  it('rejects version/method/name mismatch before execution', () => {
    for (const headers of [
      { 'MCP-Protocol-Version': '2025-11-25' },
      { 'Mcp-Method': 'tools/list' },
      { 'Mcp-Name': 'Read' },
    ]) {
      expect(negotiate('tools/call', { _meta: meta, name: 'Bash' }, headers).error?.code).toBe(-32020)
    }
  })
  it('reports unsupported revisions with supported versions', () => {
    const result = negotiate(
      'tools/list',
      { _meta: { ...meta, 'io.modelcontextprotocol/protocolVersion': '2099-01-01' } },
      { 'MCP-Protocol-Version': '2099-01-01' },
    )
    expect(result.error).toMatchObject({ code: -32022, data: { supported: [MODERN_MCP], requested: '2099-01-01' } })
  })
  it('decodes Unicode MCP names', () => {
    expect(
      negotiate(
        'tools/call',
        { _meta: meta, name: 'инструмент' },
        { 'Mcp-Name': '=?base64?' + Buffer.from('инструмент').toString('base64') + '?=' },
      ),
    ).toEqual({ modern: true })
  })
})
