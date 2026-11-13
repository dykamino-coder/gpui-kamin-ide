// INC-2026-0112: exercise argv through the real MCP transport and child, not
// just the options passed to a mocked spawn.
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { once } from 'node:events'
import { afterEach, expect, it } from 'vitest'
import type { ExternalMcpServerConfig } from '../../../shared/types'
import type { McpServerState, TransportContext } from './context'
import { connectStdio } from './stdio'

const SERVER = `
const rl = require('node:readline').createInterface({ input: process.stdin })
const send = (msg) => process.stdout.write(JSON.stringify(msg) + '\\n')
rl.on('line', (line) => {
  const msg = JSON.parse(line)
  if (msg.id === undefined) return
  if (msg.method === 'initialize') send({ jsonrpc: '2.0', id: msg.id, result: { capabilities: { tools: {} }, serverInfo: { name: 'argv-fixture', version: '1' } } })
  else if (msg.method === 'tools/list') send({ jsonrpc: '2.0', id: msg.id, result: { tools: [{ name: 'argv', description: JSON.stringify(process.argv.slice(2)), inputSchema: { type: 'object' } }] } })
  else send({ jsonrpc: '2.0', id: msg.id, result: {} })
})
rl.on('close', () => process.exit(0))
`

const directories: string[] = []
const states: McpServerState[] = []
const ARGS = ['two words', '', 'quote"inside', 'trailing\\', 'a&b|c<d>e', '(paren)^caret', '%MCP_ARG_MARKER%', '!bang!', 'кириллица']

afterEach(async () => {
  await Promise.all(states.splice(0).map(async (state) => {
    const proc = state.process
    if (!proc || proc.exitCode !== null) return
    const closed = once(proc, 'close').catch(() => {})
    proc.stdin?.end()
    await closed
  }))
  for (const directory of directories.splice(0)) fs.rmSync(directory, { recursive: true, force: true })
})

function fixture(): { directory: string; executable: string; script: string } {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'stdio argv & (fixture) %MCP_ARG_MARKER%-')))
  directories.push(directory)
  const executable = path.join(directory, process.platform === 'win32' ? 'node & (fixture).exe' : 'node & (fixture)')
  fs.copyFileSync(process.execPath, executable)
  fs.chmodSync(executable, 0o755)
  const script = path.join(directory, 'server & (fixture).cjs')
  fs.writeFileSync(script, SERVER)
  return { directory, executable, script }
}

async function received(command: string, args: string[], extra: Partial<ExternalMcpServerConfig> = {}): Promise<string[]> {
  const config = { id: `argv-${states.length}`, name: 'argv', type: 'stdio', command, args, enabled: true, ...extra }
  const state = {
    config, status: 'connecting', tools: [], toolSchemas: new Map(), resources: [], resourceTemplates: [], prompts: [],
    capabilities: {}, pendingRequests: new Map(), nextRequestId: 1, inputBuffer: '', testLog: [],
  } as unknown as McpServerState
  states.push(state)
  const ctx = {
    servers: new Map([[config.id, state]]),
    appendLog: () => {}, notifyChanged: () => {}, onToolsChanged: null,
    handleServerMessage: (_id: string, msg: { id: number }) => {
      const pending = state.pendingRequests.get(msg.id)
      state.pendingRequests.delete(msg.id)
      pending?.resolve(msg)
    },
  } as unknown as TransportContext
  await connectStdio(ctx, config.id)
  return JSON.parse(state.toolSchemas.get('argv')!.description!)
}

it('initializes an executable whose path and argv contain spaces and shell characters', async () => {
  const { executable, script } = fixture()
  expect(await received(executable, [script, ...ARGS], { env: { MCP_ARG_MARKER: 'must-not-expand' } })).toEqual(ARGS)
}, 30_000)

it('resolves an executable from the configured PATH without interpreting its argv', async () => {
  const { directory, executable, script } = fixture()
  expect(await received(path.basename(executable), [script, ...ARGS], {
    env: { PATH: `${directory}${path.delimiter}${process.env.PATH ?? ''}`, MCP_ARG_MARKER: 'must-not-expand' },
  })).toEqual(ARGS)
}, 30_000)

it('does not apply the cmd.exe line limit to a directly launched executable', async () => {
  const { executable, script } = fixture()
  const args = ['a'.repeat(9000)]
  expect(await received(executable, [script, ...args])).toEqual(args)
}, 30_000)

it.skipIf(process.platform !== 'win32').each(['cmd', 'bat'])('initializes a .%s launcher with spaces in its path and arguments', async (extension) => {
  const { directory, script } = fixture()
  const launcher = path.join(directory, `launch & (fixture).${extension}`)
  fs.writeFileSync(launcher, `@echo off\r\n"${process.execPath}" %*\r\n`)
  expect(await received(launcher, [script, ...ARGS], { env: { MCP_ARG_MARKER: 'must-not-expand' } })).toEqual(ARGS)
}, 30_000)

it.skipIf(process.platform !== 'win32')('resolves a .cmd shim through PATH as for npx', async () => {
  const { directory, script } = fixture()
  const launcher = path.join(directory, 'mcp-launcher.cmd')
  fs.writeFileSync(launcher, `@echo off\r\nSETLOCAL\r\n"${process.execPath}" %*\r\n`)
  expect(await received('mcp-launcher', [script, ...ARGS], {
    env: { PATH: `${directory}${path.delimiter}${process.env.PATH ?? ''}`, MCP_ARG_MARKER: 'must-not-expand' },
  })).toEqual(ARGS)
}, 30_000)

it.skipIf(process.platform !== 'win32')('preserves argv through the endlocal forwarding line used by npm shims', async () => {
  const { directory, script } = fixture()
  const launcher = path.join(directory, 'npm-style.cmd')
  fs.writeFileSync(launcher, `@echo off\r\nSETLOCAL\r\nSET "_prog=${process.execPath}"\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & "%_prog%" %*\r\n`)
  expect(await received('.\\npm-style.cmd', [script, ...ARGS], {
    workingDirectory: directory, env: { MCP_ARG_MARKER: 'must-not-expand' },
  })).toEqual(ARGS)
}, 30_000)
