import { expect, it, vi } from 'vitest'
const ipc = vi.hoisted(() => ({}))
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

// Exercise the actual fan-out with inert views; the constructor's startup stays out.
function fanOut() {
  const got = new Map<string, string[]>()
  const views = (['chat', 'tools', 'customize'] as const).map((role) => {
    const seen: string[] = []
    got.set(role, seen)
    return { role, view: { postMessage: async (frame: { channel: string }) => { seen.push(frame.channel); return true } } }
  })
  const host = Object.assign(Object.create(BridgeHost.prototype), {
    webviews: new Set(views.map((v) => v.view)),
    webviewRoles: new Map(views.map((v) => [v.view, v.role])),
    resync: { isHidden: () => false, markStale: () => {} },
  }) as BridgeHost
  return { host, got }
}

it('sends the commands snapshot only to chat views and the diagnostic logs to no tools view', () => {
  const f = fanOut()
  for (const channel of ['commands:snapshot', 'commands:output', 'mcp:test-log', 'mcp:test-log-cleared', 'logs:entry', 'mcp-servers-changed'])
    f.host.broadcast(channel, 'tab', {})
  expect(f.got.get('chat')).toEqual(['commands:snapshot', 'commands:output', 'mcp:test-log', 'mcp:test-log-cleared', 'logs:entry', 'mcp-servers-changed'])
  expect(f.got.get('tools')).toEqual(['mcp-servers-changed'])
  expect(f.got.get('customize')).toEqual(['mcp:test-log', 'mcp:test-log-cleared', 'logs:entry', 'mcp-servers-changed'])
})