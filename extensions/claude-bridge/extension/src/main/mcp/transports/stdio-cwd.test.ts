// INC-2026-0108: the directory a stdio MCP server starts in, observed from the
// real child started by the real transport. Without a configured directory the
// child used to inherit the extension host's directory (the application
// directory of the installed app).
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { once } from 'node:events'
import { afterEach, expect, it } from 'vitest'
import type { ExternalMcpServerConfig } from '../../../shared/types'
import { discoverProjectConfigs } from '../discovery'
import type { McpServerState, TransportContext } from './context'
import { connectStdio } from './stdio'

// Reports its own working directory as the server name of `initialize`.
const SERVER = `
const readline = require('node:readline')
const rl = readline.createInterface({ input: process.stdin })
const send = (msg) => process.stdout.write(JSON.stringify(msg) + '\\n')
rl.on('line', (line) => {
  const msg = JSON.parse(line)
  if (msg.id === undefined) return
  if (msg.method === 'initialize') send({ jsonrpc: '2.0', id: msg.id, result: { capabilities: {}, serverInfo: { name: 'cwd=' + process.cwd(), version: '1' } } })
  else if (msg.method === 'tools/list') send({ jsonrpc: '2.0', id: msg.id, result: { tools: [] } })
  else send({ jsonrpc: '2.0', id: msg.id, result: {} })
})
rl.on('close', () => process.exit(0))
`

const directories: string[] = []
const states: McpServerState[] = []
afterEach(async () => {
  await Promise.all(states.splice(0).map(async (state) => {
    const proc = state.process
    if (!proc || proc.exitCode !== null) return
    const closed = once(proc, 'close')
    proc.stdin?.end()
    await closed
  }))
  for (const directory of directories.splice(0)) fs.rmSync(directory, { recursive: true, force: true })
})

function tempDir(prefix: string): string {
  const directory = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), prefix)))
  directories.push(directory)
  return directory
}

function serverScript(): string {
  const script = path.join(tempDir('stdio-cwd-bin-'), 'server.cjs')
  fs.writeFileSync(script, SERVER)
  return script
}

/** Starts the server through `connectStdio` and returns the directory it reported. */
async function startedIn(config: ExternalMcpServerConfig): Promise<string> {
  const state = {
    config,
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
    servers: new Map([[config.id, state]]),
    appendLog: (_id: string, _level: string, msg: string) => logs.push(msg),
    notifyChanged: () => {},
    onToolsChanged: null,
    handleServerMessage: (_id: string, msg: { id: number }) => {
      const pending = state.pendingRequests.get(msg.id)
      state.pendingRequests.delete(msg.id)
      pending?.resolve(msg)
    },
  } as unknown as TransportContext
  await connectStdio(ctx, config.id)
  const line = logs.find((entry) => entry.includes('server "cwd='))
  const reported = line?.match(/server "cwd=(.*)" v1/)?.[1]
  if (!reported) throw new Error(`no cwd reported: ${logs.join('\n')}`)
  return fs.realpathSync(reported)
}

function stdioConfig(script: string, extra: Partial<ExternalMcpServerConfig> = {}): ExternalMcpServerConfig {
  return {
    id: `s-${states.length}`,
    name: 's',
    type: 'stdio',
    command: `"${process.execPath}"`,
    args: [`"${script}"`],
    enabled: true,
    ...extra,
  }
}

it('starts a server with a configured directory in that directory', async () => {
  const configured = tempDir('stdio-cwd-configured-')
  const project = tempDir('stdio-cwd-ignored-project-')
  expect(await startedIn(stdioConfig(serverScript(), { workingDirectory: configured, projectRoot: project }))).toBe(configured)
}, 30_000)

it('starts a server declared by a project .mcp.json in the project root', async () => {
  const project = tempDir('stdio-cwd-project-')
  const script = serverScript()
  fs.writeFileSync(path.join(project, '.mcp.json'), JSON.stringify({
    mcpServers: { local: { command: `"${process.execPath}"`, args: [`"${script}"`] } },
  }))
  const [config] = await discoverProjectConfigs([project])
  expect(config?.projectRoot).toBe(project)
  expect(await startedIn(config!)).toBe(project)
}, 30_000)

it('starts a user-level server in the home directory, not the host directory', async () => {
  const home = fs.realpathSync(os.homedir())
  expect(fs.realpathSync(process.cwd())).not.toBe(home)
  expect(await startedIn(stdioConfig(serverScript()))).toBe(home)
}, 30_000)

it('skips a configured directory that does not exist', async () => {
  const project = tempDir('stdio-cwd-fallback-')
  const missing = path.join(project, 'gone')
  expect(await startedIn(stdioConfig(serverScript(), { workingDirectory: missing, projectRoot: project }))).toBe(project)
}, 30_000)

it('falls back to home when the declaring project directory has disappeared', async () => {
  const missing = path.join(tempDir('stdio-cwd-missing-project-'), 'gone')
  expect(await startedIn(stdioConfig(serverScript(), { projectRoot: missing }))).toBe(fs.realpathSync(os.homedir()))
}, 30_000)

it('skips a configured path that is a file', async () => {
  const project = tempDir('stdio-cwd-file-')
  const script = serverScript()
  expect(await startedIn(stdioConfig(script, { workingDirectory: script, projectRoot: project }))).toBe(project)
}, 30_000)
