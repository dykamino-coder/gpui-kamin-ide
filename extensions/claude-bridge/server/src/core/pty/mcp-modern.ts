// MCP 2026-07-28 per-request negotiation, alongside the legacy handshake.
export const MODERN_MCP = '2026-07-28'
export const TASK_EXTENSION = 'io.modelcontextprotocol/tasks'
export function record(value: unknown): Record<string, unknown> | undefined {
  return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined
}
export function modernRequest(
  body: { method: string; params?: unknown },
  header: (name: string) => string | undefined,
): { modern: boolean; error?: { code: number; message: string; data?: unknown } } {
  const params = record(body.params)
  const meta = record(params?._meta)
  const version = meta?.['io.modelcontextprotocol/protocolVersion']
  const headerVersion = header('MCP-Protocol-Version')
  const modern =
    typeof version === 'string' ||
    (!!headerVersion && !['2024-11-05', '2025-03-26', '2025-06-18', '2025-11-25'].includes(headerVersion)) ||
    body.method === 'server/discover'
  if (!modern) return { modern: false }
  if (typeof version !== 'string' || version !== headerVersion || header('Mcp-Method') !== body.method)
    return { modern: true, error: { code: -32020, message: 'MCP header / request metadata mismatch' } }
  if (version !== MODERN_MCP)
    return {
      modern: true,
      error: {
        code: -32022,
        message: 'Unsupported protocol version',
        data: { supported: [MODERN_MCP], requested: version },
      },
    }
  if (!record(meta?.['io.modelcontextprotocol/clientCapabilities']))
    return { modern: true, error: { code: -32602, message: 'Missing clientCapabilities' } }
  const expected =
    body.method === 'resources/read'
      ? params?.uri
      : ['tools/call', 'prompts/get'].includes(body.method)
        ? params?.name
        : undefined
  if (expected !== undefined) {
    let name = header('Mcp-Name')
    if (name?.startsWith('=?base64?') && name.endsWith('?=')) {
      try {
        const encoded = name.slice(9, -2)
        const decoded = Buffer.from(encoded, 'base64')
        name =
          decoded.toString('base64') === encoded ? new TextDecoder('utf-8', { fatal: true }).decode(decoded) : undefined
      } catch {
        name = undefined
      }
    }
    if (name !== expected) return { modern: true, error: { code: -32020, message: 'MCP name header mismatch' } }
  }
  return { modern: true }
}
export function supportsTasks(params: unknown): boolean {
  const meta = record(record(params)?._meta)
  const extensions = record(record(meta?.['io.modelcontextprotocol/clientCapabilities'])?.extensions)
  return record(extensions?.[TASK_EXTENSION]) !== undefined
}
