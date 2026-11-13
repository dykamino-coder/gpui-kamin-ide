import { describe, expect, it, vi } from 'vitest'
vi.mock('vscode', () => ({ window: { showWarningMessage: vi.fn() } }))
vi.mock('../mcp/permission-manager', () => ({ PermissionManager: class {} }))
vi.mock('../mcp/executor', () => ({ executeTool: vi.fn() }))
import { ConnectionManager } from './connection-manager'

function child() {
  const send = vi.fn()
  const manager = new ConnectionManager({ serverUrl: 'ws://synthetic.invalid', token: 'synthetic' },
    { isDestroyed: () => false, webContents: { send } } as any, 'stable-tab', 'stable-conversation')
  return { manager, send, internal: manager as any }
}
describe('BR-04 replay from a replacement child', () => {
  it.each([
    { generation: 1, cached: false, cursorExpected: true },
    { generation: 2, cached: false, cursorExpected: false },
    { generation: 2, cached: true, cursorExpected: true },
  ])('seeds replacement-child history before using incremental resume: %j', async ({ generation, cached, cursorExpected }) => {
    const { manager, internal } = child()
    const cursor = { head: 'synthetic-head', pos: 100 }
    vi.spyOn(manager, 'getMirrorCursor').mockResolvedValue(cursor)
    internal.stateAuthorityGeneration = generation
    if (cached) internal.cachedJsonlEntries = [{ type: 'assistant', uuid: 'known' }]
    const socket = { readyState: 1 }
    internal.ws = socket
    const send = vi.spyOn(internal, 'sendRaw').mockImplementation(() => {})
    try {
      await internal.startSession(socket)
      expect(send.mock.calls[0]?.[0]).toMatchObject({ type: 'session:resume', history: { mode: 'paged' } })
      expect((send.mock.calls[0]?.[0] as any).history.cursor).toEqual(cursorExpected ? cursor : undefined)
    } finally {
      internal.ws = null
      manager.dispose()
    }
  })

  it.each(['disconnected', 'connecting', 'connected', 'authenticated'])('does not publish an invented empty Agents snapshot while %s', status => {
    const { manager, internal, send } = child()
    internal.state = { status }
    manager.replayJsonlToRenderer()
    expect(send.mock.calls.filter(([channel]) => channel === 'jsonl-status')).toEqual([])
    manager.dispose()
  })

  it('replays new-child authoritative Agent rows before completion, including a genuinely empty history', () => {
    const { manager, internal, send } = child()
    const rows = ['one', 'two'].map(name => ({ type: 'assistant', uuid: name,
      message: { role: 'assistant', content: [{ type: 'tool_use', name: 'Agent', id: name, input: { name } }] } }))
    internal.cachedJsonlEntries = rows
    internal.cachedJsonlStatus = { status: 'watching', replayComplete: true }
    manager.replayJsonlToRenderer()
    expect(send.mock.calls).toEqual([
      ['jsonl-status', 'stable-tab', { status: 'watching', replayComplete: false }],
      ['jsonl-entries', 'stable-tab', rows],
      ['jsonl-status', 'stable-tab', { status: 'watching', replayComplete: true }],
    ])
    send.mockClear()
    internal.cachedJsonlEntries = []
    manager.replayJsonlToRenderer()
    expect(send.mock.calls.at(-1)).toEqual(['jsonl-status', 'stable-tab', { status: 'watching', replayComplete: true }])
    manager.dispose()
  })
})
