import assert from 'node:assert/strict'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { test } from 'node:test'
import { capabilityInventory } from './capability-inventory.js'
import { assertBrowserHostReady, assertCapabilityMatrix, bridgeRoot, readMatrix } from './capability-matrix.js'
import { MCP_TOOL_NAMES } from '../../server/src/core/pty/mcp-tools-defs.js'

const inventory = capabilityInventory()
const matrix = readMatrix()

test('every API member, channel, forwarding site and client tool handler has an explicit reviewed classification', () => {
  assertCapabilityMatrix(inventory, matrix)
  const rows = new Map(matrix.rows.map(row => [row.id, row]))
  for (const name of MCP_TOOL_NAMES) assert.equal(rows.get(`tool:${name}`)?.status, 'supported-in-browser-host', `Advertised tool ${name}`)
  for (const row of matrix.rows.filter(row => row.id.startsWith('tool:') && row.status === 'explicitly-not-applicable-with-reason')) {
    assert.ok(!MCP_TOOL_NAMES.includes(row.id.slice(5)), `Excluded tool is still advertised: ${row.id}`)
  }
  // Bridge's own file viewer/context and terminal may never be excluded as IDE features.
  for (const row of matrix.rows.filter(row => /^(invoke|send|event):(file-viewer:|file-tree:|pty:|shells:list)/.test(row.id))) {
    assert.equal(row.status, 'supported-in-browser-host', row.id)
  }
  for (const id of ['api:openInVscode', 'invoke:get-editor-selection', 'event:editor-selection', 'send:vscode:open']) {
    assert.equal(rows.get(id)?.reason, 'KaminIDE host feature, not Bridge')
  }
  for (const id of ['send:workspace:terminal-sync', 'event:workspace:terminal-state', 'event:workspace:terminal-removed']) {
    assert.equal(rows.get(id)?.provider, 'terminal', id)
  }
})

test('mapping validation rejects stale rows, missing reasons, changed handlers and API route drift', () => {
  const mutate = (change: (copy: typeof matrix) => void, message: RegExp) => {
    const copy = structuredClone(matrix)
    change(copy)
    assert.throws(() => assertCapabilityMatrix(inventory, copy), message)
  }
  mutate(copy => { copy.rows.push({ ...copy.rows[0], id: 'api:stale' }) }, /Unmapped\/stale/)
  mutate(copy => { delete copy.rows.find(row => row.id === 'api:openInVscode')!.reason }, /Missing exclusion reason/)
  mutate(copy => { copy.rows.find(row => row.id === 'tool:Read')!.handler = 'writeFile' }, /Changed tool handler/)
  mutate(copy => { copy.rows.find(row => row.id === 'api:getConfig')!.routes = ['invoke:write-clipboard'] }, /Changed API routes/)
  mutate(copy => { delete copy.rows.find(row => row.id === 'tool:preview_mount')!.requires }, /Missing provider prerequisite/)
  mutate(copy => { delete copy.requirements['preview-hmr'] }, /Missing required browser-host contract/)
})

test('new source APIs, channels, dynamic dispatchers and unregistered handlers fail without explicit mapping', t => {
  const fixture = fs.mkdtempSync(path.join(os.tmpdir(), 'bridgeweb-capabilities-'))
  t.after(() => {
    assert.equal(fs.realpathSync(path.dirname(fixture)), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(fixture).startsWith('bridgeweb-capabilities-'))
    fs.rmSync(fixture, { recursive: true })
  })
  // Copy only production sources; never traverse server/data or dependencies.
  for (const directory of ['extension/src', 'webview/shared', 'webview/src', 'workspace/src', 'browser-client/src']) {
    fs.cpSync(path.join(bridgeRoot, directory), path.join(fixture, directory), {
      recursive: true,
      filter: file => fs.statSync(file).isDirectory() || /\.tsx?$/.test(file) && !/\.(test|spec)\./.test(file),
    })
  }
  const api = path.join(fixture, 'webview/shared/types.ts')
  const original = fs.readFileSync(api, 'utf8')
  fs.writeFileSync(api, original.replace('export interface KaminBridgeApi {', 'export interface KaminBridgeApi {\n futureApi(): void'))
  assert.throws(() => assertCapabilityMatrix(capabilityInventory(fixture), matrix, fixture), /api:futureApi/)
  fs.writeFileSync(api, original.replace('export interface KaminBridgeApi {', 'export interface KaminBridgeApi extends FutureApi {'))
  assert.throws(() => capabilityInventory(fixture), /Unclassified inherited API members/)
  fs.writeFileSync(api, original)
  const probe = path.join(fixture, 'extension/src/main/mcp/tools/capability-probe.ts')
  fs.writeFileSync(probe, `
    import { ipcMain } from '@kaminide/host-compat'
    ipcMain.handle('future-invoke', () => true)
    ipcMain.on('future-send', () => {})
    win.webContents.send('future-event', {})
    win.webContents.send(dynamicChannel, {})
    export async function futureHandler(input: Record<string, unknown>): Promise<McpResult> { return { content: [] } }
    export const futureArrow: ToolHandler = async input => ({ content: [] })
  `)
  const changed = capabilityInventory(fixture)
  for (const id of ['invoke:future-invoke', 'send:future-send', 'event:future-event', 'handler:futureHandler', 'handler:futureArrow',
    'forward:extension/src/main/mcp/tools/capability-probe.ts#dynamicChannel']) {
    assert.ok(changed.some(item => item.id === id), `Inventory omitted ${id}`)
  }
  assert.throws(() => assertCapabilityMatrix(changed, matrix, fixture), /future-invoke/)
  fs.writeFileSync(probe, `ipcMain.handle(unknownChannel, () => true)`)
  assert.throws(() => capabilityInventory(fixture), /Unclassified dynamic IPC registration/)
  fs.writeFileSync(probe, '')
  const adapter = path.join(fixture, 'workspace/src/capability-probe.ts')
  fs.writeFileSync(adapter, `
    const INVOKES = new Set(['future-adapter-invoke'])
    const SENDS = new Set(['future-adapter-send'])
    const EVENTS = new Set(['future-adapter-event'])
  `)
  const adapters = capabilityInventory(fixture)
  for (const id of ['invoke:future-adapter-invoke', 'send:future-adapter-send', 'event:future-adapter-event']) {
    assert.ok(adapters.some(item => item.id === id), `Inventory omitted dispatch set ${id}`)
  }
  assert.throws(() => assertCapabilityMatrix(adapters, matrix, fixture), /future-adapter-invoke/)
  fs.writeFileSync(adapter, `
    if ('future-reversed-route' === frame.channel) {}
    switch (frame.channel) {
      case 'future-switch-route': break
      default: break
    }
  `)
  const routes = capabilityInventory(fixture)
  for (const id of ['route:future-reversed-route', 'route:future-switch-route']) {
    assert.ok(routes.some(item => item.id === id), `Inventory omitted dispatcher ${id}`)
  }
  assert.throws(() => assertCapabilityMatrix(routes, matrix, fixture), /future-reversed-route/)
  fs.writeFileSync(adapter, 'switch (frame.channel) { case unknownChannel: break }')
  assert.throws(() => capabilityInventory(fixture), /Unclassified channel case/)
})

test('declared readiness rejects pending adapters and cannot be bypassed by deleting requirements', () => {
  const copy = structuredClone(matrix)
  copy.requirements['synthetic-missing-adapter'] = { state: 'pending', owner: 'fixture', reason: 'Required real host implementation' }
  assert.throws(() => assertBrowserHostReady(copy), /synthetic-missing-adapter/)
  assert.throws(() => assertBrowserHostReady({ ...copy, requirements: {} }), /Missing required browser-host contract/)
  for (const id of ['host-terminal', 'title-regeneration', 'finished-toast-route', 'preview-hmr']) {
    const missing = structuredClone(matrix)
    delete missing.requirements[id]
    assert.throws(() => assertBrowserHostReady(missing), new RegExp(`Missing required browser-host contract: ${id}`))
    assert.throws(() => assertCapabilityMatrix(inventory, missing), new RegExp(`Missing required browser-host contract: ${id}`))
  }
  assert.doesNotThrow(() => assertBrowserHostReady(matrix))
})
