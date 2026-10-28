import { afterEach, expect, it, vi } from 'vitest'
import { mcpDiagnostic } from './mcp-diagnostics'

afterEach(() => { vi.unstubAllEnvs(); vi.restoreAllMocks() })

it('emits only explicit correlation metadata when enabled', () => {
  const log = vi.spyOn(console, 'info').mockImplementation(() => {})
  vi.stubEnv('KAMIN_BRIDGE_MCP_DIAGNOSTICS', '')
  mcpDiagnostic('received', { requestId: 'synthetic' })
  expect(log).not.toHaveBeenCalled()
  vi.stubEnv('KAMIN_BRIDGE_MCP_DIAGNOSTICS', '1')
  mcpDiagnostic('received', { requestId: 'synthetic', queuedBytes: 123 })
  expect(JSON.parse(log.mock.calls[0]![1])).toEqual({ timestamp: expect.any(Number), phase: 'received', requestId: 'synthetic', queuedBytes: 123 })
})
