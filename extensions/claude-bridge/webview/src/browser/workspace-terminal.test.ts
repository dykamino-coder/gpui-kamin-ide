import { afterEach, describe, expect, it, vi } from 'vitest'
import { Terminal } from '@xterm/headless'
import type { WebviewApi } from '../lib/webview-api'
import type { KaminBridgeApi } from '../../shared/types'
import { attachWorkspaceTerminal } from './workspace-terminal'
import { guardTerminalQueries } from './terminal-query-guards'

const terminals: Terminal[] = []
const projections: Array<{ dispose(): void }> = []
afterEach(() => { for (const projection of projections.splice(0)) projection.dispose(); for (const terminal of terminals.splice(0)) terminal.dispose() })
function create() {
  const terminal = new Terminal({ cols: 80, rows: 24, allowProposedApi: true })
  terminals.push(terminal)
  return terminal
}
function flush(terminal: Terminal) { return new Promise<void>(resolve => terminal.write('', resolve)) }
function fixture() {
  const messages = new Set<(frame: unknown) => void>(), disconnects = new Set<() => void>()
  const postMessage = vi.fn(), ptySpawn = vi.fn<KaminBridgeApi['ptySpawn']>(async () => ({
    ptyId: 'new', profile: { id: 'sh', label: 'Shell', command: 'sh', args: [] },
  })), ptyKill = vi.fn()
  const api: WebviewApi = { postMessage, getState: () => undefined, setState: () => {},
    onHostMessage: cb => { messages.add(cb); return () => { messages.delete(cb) } },
    onDisconnect: cb => { disconnects.add(cb); return () => { disconnects.delete(cb) } },
  }
  const terminal = create(), onIdentity = vi.fn(), onProfile = vi.fn()
  // Restoration must disable input before reset/resize, which fire local callbacks.
  terminal.onResize(() => { expect(onIdentity.mock.lastCall).toEqual([null]) })
  const projection = attachWorkspaceTerminal({ api, bridge: { ptySpawn, ptyKill },
    tabId: 'chat', cwd: '/workspace', terminal, onIdentity, onProfile })
  projections.push(projection)
  const event = (channel: string, ...args: unknown[]) => { for (const cb of messages) cb({ kind: 'event', channel, args }) }
  const snapshot = (overrides = {}) => event('workspace:terminal-state', {
    ptyId: 'existing', profile: { id: 'sh' }, cols: 93, rows: 31, sequence: 10, screen: 'host-history', ...overrides,
  }, 'chat')
  return { terminal, projection, postMessage, ptySpawn, ptyKill, event, snapshot, onIdentity, onProfile,
    disconnect: () => { for (const cb of disconnects) cb() } }
}

describe('browser projection of host-owned file-panel terminal', () => {
  it('restores real xterm history and sequences deltas without process mutations', async () => {
    const f = fixture()
    f.snapshot()
    f.event('pty:data', { ptyId: 'existing', sequence: 10, data: 'DUPLICATE' })
    f.event('pty:data', { ptyId: 'other', sequence: 11, data: 'OTHER' })
    f.event('pty:data', { ptyId: 'existing', sequence: 11, data: '-delta' })
    f.snapshot({ sequence: 9, screen: 'STALE' })
    await vi.waitFor(() => expect(f.terminal.buffer.active.getLine(0)?.translateToString(true)).toBe('host-history-delta'))
    expect([f.terminal.cols, f.terminal.rows]).toEqual([93, 31])
    expect(f.onIdentity.mock.lastCall).toEqual(['existing'])
    expect(f.ptySpawn).not.toHaveBeenCalled()
    expect(f.ptyKill).not.toHaveBeenCalled()
    expect(f.postMessage.mock.calls).toEqual([[{ kind: 'send', channel: 'workspace:terminal-sync', args: ['chat'] }]])
  })

  it('reconnect requests only a private snapshot and unmount never kills a shell', async () => {
    const f = fixture()
    f.snapshot(); f.disconnect()
    f.event('pty:data', { ptyId: 'existing', sequence: 11, data: 'OFFLINE' })
    f.event('bridge:reconnected')
    f.snapshot({ sequence: 20, screen: 'zero-page-output', exitCode: 7 })
    await vi.waitFor(() => expect(f.terminal.buffer.active.getLine(0)?.translateToString(true)).toBe('zero-page-output'))
    expect(f.onIdentity.mock.lastCall).toEqual([null])
    f.projection.dispose()
    f.event('bridge:reconnected')
    expect(f.postMessage.mock.calls).toHaveLength(2)
    expect(f.postMessage.mock.calls.every(([frame]) => frame.channel === 'workspace:terminal-sync')).toBe(true)
    expect(f.ptySpawn).not.toHaveBeenCalled()
    expect(f.ptyKill).not.toHaveBeenCalled()
  })

  it('creates once on first empty mount, recovers a lost reply and never retries on reconnect', async () => {
    const f = fixture()
    f.ptySpawn.mockRejectedValueOnce(new Error('Connection lost'))
    f.event('workspace:terminal-state', null, 'chat')
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    f.disconnect(); f.event('bridge:reconnected'); f.snapshot()
    expect(f.ptySpawn).toHaveBeenCalledTimes(1)
    expect(f.ptySpawn).toHaveBeenCalledWith({ tabId: 'chat', shellId: undefined, cwd: '/workspace', cols: 80, rows: 24 })
    f.event('workspace:terminal-state', null, 'chat')
    expect(f.ptySpawn).toHaveBeenCalledTimes(1)
    expect(f.ptyKill).not.toHaveBeenCalled()
    f.projection.dispose()
  })

  it('only explicit shell selection replaces a process; other tabs and removed records stay inert', async () => {
    const f = fixture()
    f.event('workspace:terminal-state', null, 'different-chat')
    f.snapshot()
    f.projection.selectShell('bash')
    await Promise.resolve(); await Promise.resolve(); await Promise.resolve()
    expect(f.ptyKill).toHaveBeenCalledExactlyOnceWith('existing')
    expect(f.ptySpawn).toHaveBeenCalledTimes(1)
    expect(f.ptySpawn.mock.calls[0]?.[0]).toMatchObject({ tabId: 'chat', shellId: 'bash' })
    f.snapshot({ ptyId: 'new', sequence: 0 })
    f.event('workspace:terminal-removed', { ptyId: 'new', tabId: 'chat' })
    f.event('workspace:terminal-state', null, 'chat')
    expect(f.onIdentity.mock.lastCall).toEqual([null])
    expect(f.ptySpawn).toHaveBeenCalledTimes(1)
    f.projection.dispose()
  })

  it('two replicas suppress parser query replies while preserving user input', async () => {
    const queries = '\x1b[c\x1b[>c\x1b[6n\x1b[?6n\x1b[?25$p\x1bP$qm\x1b\\'
    const source = create(), sourceReplies: string[] = []
    source.onData(data => sourceReplies.push(data))
    source.write(queries); await flush(source)
    expect(sourceReplies.length).toBeGreaterThanOrEqual(5)
    for (let i = 0; i < 2; i++) {
      const replica = create(), replies: string[] = []
      const off = guardTerminalQueries(replica.parser)
      replica.onData(data => replies.push(data))
      replica.write(queries); await flush(replica)
      expect(replies).toEqual([])
      replica.input('user-input', true)
      expect(replies).toEqual(['user-input'])
      off()
    }
  })
})
