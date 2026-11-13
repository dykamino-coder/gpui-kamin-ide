// A JSON-RPC frame whose UTF-8 bytes arrive split inside a character must
// decode intact: per-chunk decoding turned Cyrillic into U+FFFD.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, expect, it } from 'vitest'
import type { McpServerState, TransportContext } from './context'
import { connectStdio } from './stdio'

const DESCRIPTION = 'Инструмент: проверка — ' + 'ЖЖЖ'.repeat(2000)

// Answers initialize and tools/list; each reply is written in two parts cut
// in the middle of a two-byte character, with a pause between them.
const SERVER = `
const readline = require('node:readline')
const rl = readline.createInterface({ input: process.stdin })
const send = (msg) => {
  const bytes = Buffer.from(JSON.stringify(msg) + '\\n', 'utf8')
  let cut = bytes.indexOf(Buffer.from('Ж', 'utf8')) + 1
  if (cut <= 0) cut = bytes.length
  process.stdout.write(bytes.subarray(0, cut))
  setTimeout(() => process.stdout.write(bytes.subarray(cut)), 30)
}
rl.on('line', (line) => {
  const msg = JSON.parse(line)
  if (msg.id === undefined) return
  if (msg.method === 'initialize') send({ jsonrpc: '2.0', id: msg.id, result: { capabilities: {}, serverInfo: { name: 'Сервер Ж', version: '1' } } })
  else if (msg.method === 'tools/list') send({ jsonrpc: '2.0', id: msg.id, result: { tools: [{ name: 'echo', description: ${JSON.stringify(DESCRIPTION)}, inputSchema: { type: 'object' } }] } })
  else send({ jsonrpc: '2.0', id: msg.id, result: {} })
})
`

const directories: string[] = []
const states: McpServerState[] = []
afterEach(() => {
  for (const state of states.splice(0)) state.process?.kill()
  for (const directory of directories.splice(0)) fs.rmSync(directory, { recursive: true, force: true })
})

it('decodes a JSON-RPC frame split inside a multi-byte character', async () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'stdio-utf8-'))
  directories.push(directory)
  const script = path.join(directory, 'server.cjs')
  fs.writeFileSync(script, SERVER)
  const state = {
    config: { id: 's', name: 's', type: 'stdio', command: process.execPath, args: [script], enabled: true },
    status: 'connecting',
    tools: [],
    toolSchemas: new Map(),
    resources: [],
    resourceTemplates: [],
    prompts: [],
    capabilities: {},
    pendingRequests: new Map(),
    nextRequestId: 1,
    inputBuffer: '',
    testLog: [],
  } as unknown as McpServerState
  states.push(state)
  const logs: string[] = []
  const ctx = {
    servers: new Map([['s', state]]),
    appendLog: (_id: string, _level: string, msg: string) => logs.push(msg),
    notifyChanged: () => {},
    onToolsChanged: null,
    handleServerMessage: (_id: string, msg: { id: number }) => {
      const pending = state.pendingRequests.get(msg.id)
      state.pendingRequests.delete(msg.id)
      pending?.resolve(msg)
    },
  } as unknown as TransportContext

  await connectStdio(ctx, 's')
  expect(state.toolSchemas.get('echo')?.description).toBe(DESCRIPTION)
  expect(logs.join('\n')).toContain('server "Сервер Ж"')
  expect(logs.join('\n')).not.toContain('�')
}, 30_000)
