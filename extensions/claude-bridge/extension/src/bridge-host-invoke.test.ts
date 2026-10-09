import { describe, expect, it, vi } from 'vitest'
const ipc = vi.hoisted(() => ({ invokeHandler: vi.fn(), hasHandler: vi.fn(() => true) }))
vi.mock('vscode', () => ({}))
vi.mock('@kaminide/host-compat', () => ({ ipcMain: ipc }))
vi.mock('./main/tab-manager', () => ({}))
vi.mock('./main/config/store', () => ({}))
vi.mock('./main/ipc/sessions', () => ({}))
vi.mock('./main/ipc/config', () => ({}))
vi.mock('./main/ipc/tabs', () => ({}))
vi.mock('./main/ipc/skills-agents', () => ({}))
vi.mock('./main/ipc/hooks', () => ({}))
vi.mock('./main/ipc/logs', () => ({}))
vi.mock('./main/ipc/mcp', () => ({}))
vi.mock('./main/ipc/plugins', () => ({}))
vi.mock('./main/ipc/marketplaces', () => ({}))
vi.mock('./main/ipc/monitors', () => ({}))
vi.mock('./main/plugin-monitors', () => ({}))
vi.mock('./main/plugin-lsp', () => ({}))
vi.mock('./main/sync/sync-client', () => ({}))
vi.mock('./main/error-log', () => ({}))
vi.mock('./main/mcp/tools/ui-tools', () => ({}))
vi.mock('./main/notifications/toast-window', () => ({}))
vi.mock('./main/mcp/permission-manager', () => ({}))
vi.mock('./main/mcp/manager', () => ({}))
vi.mock('./main/mcp/executor', () => ({}))
vi.mock('./main/hooks/emit-bridge-event', () => ({}))
vi.mock('./main/ws/connection-manager', () => ({}))
vi.mock('./core-ipc', () => ({}))
vi.mock('./main/ipc/sync', () => ({}))
vi.mock('./incident-diagnostics', () => ({}))

import { BridgeHost } from './bridge-host'
import { InvokeDiagnostics, type SafeInvokeBoundary } from './invoke-diagnostics'

function actualRouter(postMessage: (frame: any) => Promise<boolean>) {
  const records: SafeInvokeBoundary[] = []
  const source = { postMessage }
  const observer = new InvokeDiagnostics<object>(event => records.push(event))
  observer.attach(source)
  // Exercise the actual router method; constructor's service/network startup is
  // intentionally outside this inert fixture and never touches an owner profile.
  const host = Object.assign(Object.create(BridgeHost.prototype), { event: { sender: {} }, invokeDiagnostics: observer })
  return { host, source, records, observer }
}
describe('BR-24 actual BridgeHost.onMessage route', () => {
  it('records a successful store mutation and one false reply send, without repeating mutation', async () => {
    const frames: any[] = []
    const f = actualRouter(async frame => { frames.push(frame); return false })
    let writes = 0
    ipc.invokeHandler.mockImplementation(async () => { writes++; return { privateResult: 'synthetic-result' } })
    ipc.hasHandler.mockReturnValue(true)
    await f.host.onMessage({ kind: 'invoke', id: 7, channel: 'hooks:set-plugin-approval', generation: 'synthetic-doc', args: ['synthetic-private-argument'] }, f.source)
    expect(writes).toBe(1)
    expect(frames).toHaveLength(1)
    expect(frames[0]).toMatchObject({ kind: 'invoke-reply', id: 7, generation: 'synthetic-doc', ok: true })
    expect(f.records.map(record => record.stage)).toEqual(['host-received', 'handler-resolved', 'reply-false'])
    expect(JSON.stringify(f.records)).not.toContain('synthetic-')
  })
  it('records handler error and rejected reply independently without leaking either error', async () => {
    const f = actualRouter(async () => { throw new Error('private-send-error') })
    ipc.invokeHandler.mockRejectedValueOnce(new Error('private-handler-error'))
    ipc.hasHandler.mockReturnValue(true)
    await f.host.onMessage({ kind: 'invoke', id: 8, channel: 'get-config', generation: 'synthetic-doc' }, f.source)
    expect(f.records.map(record => record.stage)).toEqual(['host-received', 'handler-rejected', 'reply-rejected'])
    expect(JSON.stringify(f.records)).not.toContain('private-')
  })
  it('identifies an unfinished handler and a reply after document replacement through the real route', async () => {
    const f = actualRouter(async () => true)
    let resolve!: (value: unknown) => void
    ipc.invokeHandler.mockImplementationOnce(() => new Promise(r => { resolve = r }))
    const pending = f.host.onMessage({ kind: 'invoke', id: 1, channel: 'get-config', generation: 'old' }, f.source)
    expect(f.records.map(record => record.stage)).toEqual(['host-received'])
    await f.host.onMessage({ kind: 'invoke-diagnostic', id: 1, channel: 'get-config', generation: 'new', stage: 'renderer-sent', args: ['private'] }, f.source)
    resolve('private-result'); await pending
    expect(f.records.map(record => record.stage)).toEqual(['host-received', 'renderer-sent', 'handler-resolved', 'reply-document-replaced', 'reply-accepted'])
    expect(JSON.stringify(f.records)).not.toContain('private')
  })
})
