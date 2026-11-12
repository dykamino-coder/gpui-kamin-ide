// Mandatory gate for every pinned codex version. With Bridge's configuration:
//   - the model is offered ONLY Bridge MCP tools, provider-side web search and
//     codex's MCP-resource / request_user_input helpers — both as direct tools
//     and, for code-mode models (gpt-6 family), as tools nested in `exec`;
//   - built-in exec/shell/apply_patch calls are refused without side effects;
//   - one app-server serves parallel Bridge sessions: each thread reaches its
//     own MCP endpoint with its own bearer, nothing leaks between threads, and
//     the bearer never lands in CODEX_HOME;
//   - code-mode MCP calls surface as normal tool events;
//   - model traffic goes through the configured proxy, MCP traffic to
//     Bridge's loopback endpoint does not.
//
// Needs the real codex binary, so it runs only with BRIDGE_CODEX_GATE=1
// (optionally BRIDGE_CODEX_BIN). The Bridge server CI job installs the npm
// package at the Dockerfile's ARG CODEX_VERSION and runs it as a merge gate;
// the plain suite keeps protocol behaviour covered by recorded fixtures.
// Fully offline: the Responses API and MCP peers are local mocks.
import { execFile } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { promisify } from 'node:util'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { startCodexMock } from '../../../test-support/codex-mock.mjs'
import { startTestProxy } from '../../../test-support/http-proxy.mjs'
import type { AgentEvent } from '../../shared/agent-events'
import { resolveCodexCommand } from './binary'
import { buildCodexEnv, prepareCodexHome } from './codex-home'
import { CodexHost } from './codex-host'
import { CodexAgentSession } from './codex-session'
import { bridgeMcpThreadConfig, codexLockdownConfig, mergeTables, toConfigOverrides } from './config-toml'
import { buildPatchedCatalog } from './model-catalog'

const enabled = process.env.BRIDGE_CODEX_GATE === '1'
const MODEL_HOST = 'model.codex-mock.test'
type Key = 'a' | 'b' | 'c'
const SESSIONS: Record<Key, { model: string; token: string }> = {
  a: { model: 'gpt-5.5', token: 'gate-token-a-not-a-secret' },
  b: { model: 'gpt-5.5', token: 'gate-token-b-not-a-secret' },
  // A code_mode_only model: tools are reachable only from JavaScript in `exec`.
  c: { model: 'gpt-6-sol', token: 'gate-token-c-not-a-secret' },
}

/** Every tool name a Bridge codex session may offer the model. */
const ALLOWED = new Set([
  'list_mcp_resources',
  'list_mcp_resource_templates',
  'read_mcp_resource',
  'request_user_input',
  'web_search',
  // code mode: the JS runner and its yield helper; nested tools are checked separately
  'exec',
  'wait',
])
const ALLOWED_NAMESPACES = {
  direct: { mcp__bridge: ['Bash', 'Read'], mcp__bridge_interactive: ['AskUserQuestion'] },
}
const NESTED_IN_EXEC = [
  'list_mcp_resource_templates',
  'list_mcp_resources',
  'mcp__bridge__Bash',
  'mcp__bridge__Read',
  'mcp__bridge_interactive__AskUserQuestion',
  'read_mcp_resource',
]
const FORBIDDEN = /exec_command|write_stdin|^shell$|apply_patch|view_image|image_gen|spawn_agent|update_plan/

const MCP_TOOLS = [
  { name: 'Read', description: 'synthetic', inputSchema: { type: 'object', properties: { path: { type: 'string' } } } },
  {
    name: 'Bash',
    description: 'synthetic',
    inputSchema: { type: 'object', properties: { command: { type: 'string' } } },
  },
  { name: 'AskUserQuestion', description: 'synthetic', inputSchema: { type: 'object', properties: {} } },
]

/** Tools from both channels: top-level `tools` and code mode's `additional_tools` input item. */
// Names only: the xec description itself shows 	ools.exec_command(...) as a usage example.
function offeredTools(body: any): { names: string[]; namespaces: Record<string, string[]> } {
  const all = [...(body.tools ?? [])]
  for (const item of body.input ?? []) if (item.type === 'additional_tools') all.push(...(item.tools ?? []))
  const names: string[] = []
  const namespaces: Record<string, string[]> = {}
  for (const t of all) {
    if (t.type === 'namespace' && String(t.name).startsWith('mcp__')) {
      namespaces[t.name] = (t.tools ?? []).map((x: any) => x.name).sort()
    } else if (t.type === 'namespace') {
      names.push(...(t.tools ?? []).map((x: any) => x.name))
    } else {
      names.push(t.name ?? t.type)
    }
  }
  return { names, namespaces }
}

function outputsOf(body: any): Map<string, string> {
  const out = new Map<string, string>()
  for (const item of body.input ?? []) {
    if (item.type === 'function_call_output' || item.type === 'custom_tool_call_output') {
      out.set(item.call_id, typeof item.output === 'string' ? item.output : JSON.stringify(item.output))
    }
  }
  return out
}

function filesContaining(dir: string, needle: string): string[] {
  const hits: string[] = []
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name)
    if (entry.isDirectory()) hits.push(...filesContaining(p, needle))
    else if (fs.readFileSync(p).includes(needle)) hits.push(p)
  }
  return hits
}

describe.runIf(enabled)('codex tool gate (real codex binary, offline mocks)', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-gate-'))
  const home = path.join(root, 'home')
  const cwd = path.join(root, 'ws')
  const command = resolveCodexCommand()
  let mock: Awaited<ReturnType<typeof startCodexMock>>
  let proxy: Awaited<ReturnType<typeof startTestProxy>>
  let host: CodexHost
  const events: Record<Key, AgentEvent[]> = { a: [], b: [], c: [] }
  const requestsOf = (key: Key) =>
    mock.responsesRequests.filter((r) => JSON.stringify(r.body.input).includes(`probe-${key}`))

  beforeAll(async () => {
    fs.mkdirSync(cwd, { recursive: true })
    fs.mkdirSync(home, { recursive: true })
    const catalog = await buildPatchedCatalog(async (args) => {
      const { stdout } = await promisify(execFile)(command.command, [...command.args, ...args], {
        env: buildCodexEnv({ home }),
        maxBuffer: 64 * 1024 * 1024,
        windowsHide: true,
      })
      return stdout
    })
    const prepared = prepareCodexHome(home, catalog)
    mock = await startCodexMock({
      mcpTools: MCP_TOOLS,
      scripts: {
        // a: every refused built-in, one Bridge MCP call, then reasoning + text.
        'probe-a': [
          [{ type: 'call', name: 'exec_command', callId: 'c_exec', arguments: { cmd: 'echo pwn > exec.txt' } }],
          [{ type: 'call', name: 'shell', callId: 'c_shell', arguments: { command: ['echo', 'pwn'], workdir: '.' } }],
          [
            {
              type: 'call',
              name: 'apply_patch',
              callId: 'c_patch',
              arguments: { input: '*** Begin Patch\n*** Add File: patch.txt\n+pwn\n*** End Patch\n' },
            },
          ],
          [{ type: 'mcp-call', name: 'Read', callId: 'c_read', arguments: { path: 'a.txt' } }],
          [
            { type: 'reasoning', summary: ['Thinking about it'] },
            { type: 'message', chunks: ['al', 'l ', 'done'] },
          ],
        ],
        // b runs in parallel with a, against its own endpoint.
        'probe-b': [
          [{ type: 'mcp-call', name: 'Bash', callId: 'c_bash', arguments: { command: 'dir' } }],
          [{ type: 'message', text: 'b done' }],
        ],
        // c: code mode lists its nested tools, calls Bridge MCP, then tries a built-in.
        'probe-c': [
          [
            {
              type: 'custom',
              name: 'exec',
              namespace: 'functions',
              callId: 'c_code',
              input:
                "text(Object.keys(tools).sort().join(','));" +
                "await tools.mcp__bridge__Read({ path: 'c.txt' });" +
                "await tools.exec_command({ cmd: 'echo pwn > code.txt' });",
            },
          ],
          [{ type: 'message', text: 'c done' }],
        ],
      },
    })
    proxy = await startTestProxy({ hosts: { [MODEL_HOST]: `127.0.0.1:${mock.port}` } })
    const processConfig = mergeTables(codexLockdownConfig({ catalogPath: prepared.catalogPath }), {
      model_provider: 'mock',
      model_providers: {
        mock: { name: 'mock', base_url: `http://${MODEL_HOST}/v1`, wire_api: 'responses', env_key: 'MOCK_KEY' },
      },
    })
    host = await CodexHost.start({
      command,
      cwd,
      env: buildCodexEnv({
        home,
        injectProxy: (env) => {
          env.HTTP_PROXY = proxy.url
          env.HTTPS_PROXY = proxy.url
        },
        extra: { MOCK_KEY: 'mock-key-not-a-secret' },
      }),
      configOverrides: toConfigOverrides(processConfig),
      // Raw protocol capture for refreshing ./fixtures (sanitize before committing).
      onTrace: process.env.BRIDGE_CODEX_TRACE
        ? (direction, message) =>
            fs.appendFileSync(process.env.BRIDGE_CODEX_TRACE!, JSON.stringify({ direction, message }) + '\n')
        : undefined,
    })
    await Promise.all(
      (Object.keys(SESSIONS) as Key[]).map(async (key) => {
        const session = await CodexAgentSession.start({
          host,
          cwd,
          model: SESSIONS[key].model,
          threadConfig: bridgeMcpThreadConfig({ url: mock.mcpUrl(`session-${key}`), token: SESSIONS[key].token }),
        })
        session.events.subscribe((event) => events[key].push(event))
        await session.send(`probe-${key}`)
        await session.waitForIdle(60_000)
        await session.stop()
      }),
    )
    // Closed before the assertions so sqlite/rollout files are flushed and unlocked.
    expect(host.threadCount).toBe(0)
    await host.close()
  }, 180_000)

  afterAll(async () => {
    await host?.close()
    await mock?.close()
    await proxy?.close()
    fs.rmSync(root, { recursive: true, force: true })
  })

  it('offers the model only Bridge MCP tools, web search and service helpers', () => {
    expect(mock.responsesRequests).toHaveLength(9)
    for (const request of mock.responsesRequests) {
      const { names, namespaces } = offeredTools(request.body)
      for (const name of names) expect(ALLOWED, `unexpected tool ${name}`).toContain(name)
      for (const name of [...names, ...Object.values(namespaces).flat()]) expect(name).not.toMatch(FORBIDDEN)
      if (!names.includes('exec')) expect(namespaces).toEqual(ALLOWED_NAMESPACES.direct)
    }
    expect(offeredTools(requestsOf('a')[0]!.body).names).toContain('web_search')
  })

  it('exposes exactly the Bridge tools inside code mode', () => {
    const output = outputsOf(requestsOf('c').at(-1)!.body).get('c_code') ?? ''
    expect(output).toContain(NESTED_IN_EXEC.join(','))
    expect(output).toMatch(/tools\.exec_command is not a function/)
    expect(events.c.find((e) => e.kind === 'tool.completed')).toMatchObject({
      tool: { source: 'mcp', server: 'bridge', name: 'Read' },
      input: { path: 'c.txt' },
      status: 'completed',
    })
  })

  it('refuses exec_command, shell and apply_patch without side effects', () => {
    for (const file of ['exec.txt', 'shell.txt', 'patch.txt', 'code.txt']) {
      expect(fs.existsSync(path.join(cwd, file))).toBe(false)
    }
    const outputs = new Map<string, string>()
    for (const request of requestsOf('a')) for (const [k, v] of outputsOf(request.body)) outputs.set(k, v)
    for (const id of ['c_exec', 'c_shell', 'c_patch']) expect(outputs.get(id)).toMatch(/unsupported call/i)
    for (const list of Object.values(events)) {
      expect(list.some((e) => e.kind === 'tool.started' && e.tool.source === 'builtin')).toBe(false)
    }
  })

  it('keeps parallel threads on their own MCP endpoint, bearer and event stream', () => {
    const calls = mock.mcpRequests.filter((r) => r.body?.method === 'tools/call')
    expect(calls.map((c) => `${c.path} ${c.body.params.name}`).sort()).toEqual([
      '/mcp/session-a Read',
      '/mcp/session-b Bash',
      '/mcp/session-c Read',
    ])
    expect(mock.mcpRequests.every((r) => r.authorized)).toBe(true)
    for (const key of Object.keys(SESSIONS) as Key[]) {
      const own = mock.mcpRequests.filter((r) => r.path === `/mcp/session-${key}`)
      expect(own.every((r) => r.authorization === `Bearer ${SESSIONS[key].token}`)).toBe(true)
      const threadIds = new Set(events[key].map((e) => e.threadId))
      expect(threadIds.size).toBe(1)
    }
    const threadOf = (key: Key) => events[key][0]!.threadId
    expect(new Set([threadOf('a'), threadOf('b'), threadOf('c')]).size).toBe(3)
    expect(events.b.find((e) => e.kind === 'tool.completed')).toMatchObject({ tool: { name: 'Bash' } })
    expect(events.a.some((e) => e.kind === 'message.assistant' && e.text !== 'all done')).toBe(false)
  })

  it('keeps MCP bearer tokens out of CODEX_HOME', () => {
    for (const { token } of Object.values(SESSIONS)) expect(filesContaining(home, token)).toEqual([])
  })

  it('sends model traffic through the proxy and MCP traffic around it', () => {
    const viaProxy = proxy.seen.map((s) => s.target)
    expect(viaProxy.filter((t) => t === `${MODEL_HOST}/v1/responses`)).toHaveLength(9)
    expect(viaProxy.some((t) => t.includes('/mcp/'))).toBe(false)
    expect(mock.mcpRequests.length).toBeGreaterThan(0)
  })

  it('streams reasoning, assistant text, usage and one completed turn per session', () => {
    const a = events.a
    expect(a.map((e) => e.kind)).toContain('reasoning')
    expect(
      a
        .filter((e) => e.kind === 'message.assistant.delta')
        .map((e: any) => e.delta)
        .join(''),
    ).toBe('all done')
    expect(a.find((e) => e.kind === 'message.assistant')).toMatchObject({ text: 'all done' })
    expect(a.map((e) => e.kind)).toContain('usage')
    for (const list of Object.values(events)) {
      expect(list.filter((e) => e.kind === 'turn.completed')).toEqual([
        expect.objectContaining({ status: 'completed' }),
      ])
      expect(list.at(-1)).toMatchObject({ kind: 'session.exited', reason: 'stopped' })
    }
  })
})
