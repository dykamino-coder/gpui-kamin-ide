// INC-2026-0097: the client host declares what it can execute; the server serves
// exactly that. In-memory HTTP routing only: no Bridge listener or Claude process.
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { Hono } from 'hono'
import type { PtySession } from './types'

const mocks = vi.hoisted(() => ({ getSession: vi.fn(), call: vi.fn() }))
vi.mock('./session-manager', () => ({ getSession: mocks.getSession, sendMcpCall: mocks.call }))
vi.mock('./session-ws', () => ({ getSessionWs: vi.fn() }))
vi.mock('./session-io', () => ({ sendToClient: vi.fn() }))
vi.mock('../events/bus', () => ({ eventBus: { emit: vi.fn() } }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))

import { handleMcpRequest } from './mcp-http-handler'
import { MCP_TOOLS } from './mcp-tools-defs'
import { mcpToolsFor, normalizeHostDeclaration, sameHostDeclaration, type HostDeclaration } from './host-declaration'
import { mcpInstructions } from './mcp-instructions'
import { BRIDGE_DEFAULT_CLAUDE_MD, bridgeClaudeMd } from './bridge-default-claude-md'
import { buildSystemPrompt } from './system-prompt'

const ALL = MCP_TOOLS.map((tool) => tool.name)
const without = (...names: string[]) => ALL.filter((name) => !names.includes(name))
const declare = (tools: string[], platform: string, place: string) =>
  normalizeHostDeclaration({ tools, platform, place }) as HostDeclaration

// The four hosts the contract distinguishes.
const linuxNoPwsh = declare(without('PowerShell'), 'linux', 'the workspace host')
const windows = declare(without('preview_mount', 'preview_update', 'preview_unmount'), 'win32', "the user's machine")
const noShell = declare(['Read', 'Glob', 'Grep', 'AskUserQuestion'], 'linux', 'the review host')
const LEGACY_WORDING = /user's (machine|Windows host|filesystem|network|OS|project)|their own machine/

const app = new Hono().post('/mcp/:sessionId', handleMcpRequest)
let session: PtySession

function request(method: string, params: Record<string, unknown> = {}) {
  return app.request('/mcp/synthetic-session', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      authorization: 'Bearer synthetic-token',
      'MCP-Protocol-Version': '2025-11-25',
      'Mcp-Method': method,
    },
    body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
  })
}
async function listed(host?: HostDeclaration): Promise<Array<{ name: string; description: string }>> {
  session.host = host
  return (await (await request('tools/list')).json()).result.tools
}

// Server-side history tools are always listed after the host's own.
const builtin = (tools: Array<{ name: string }>) =>
  tools.map((tool) => tool.name).filter((name) => !name.startsWith('project_session'))

beforeEach(() => {
  vi.clearAllMocks()
  session = {
    id: 'synthetic-session',
    mcpToken: 'synthetic-token',
    mcpInitialized: true,
    mcpLastError: null,
    mcpLog: [],
    mcpCallCount: 0,
    lastActivityAt: new Date(),
    registeredTools: [],
    ws: { readyState: 1 },
  } as unknown as PtySession
  mocks.getSession.mockReturnValue(session)
  mocks.call.mockResolvedValue('ordinary result')
})

describe('declaration normalization', () => {
  it('treats anything that is not a declaration as no declaration', () => {
    for (const raw of [undefined, null, 'linux', [], {}, { tools: 'Bash' }])
      expect(normalizeHostDeclaration(raw)).toBeUndefined()
  })

  it('keeps only known built-in tools, a platform token and a one-line place', () => {
    const host = normalizeHostDeclaration({
      tools: ['Grep', 'Invented', 7, 'Read', 'Read'],
      platform: 'linux; rm',
      place: '  the `build`\n host  ',
    })
    expect(host).toEqual({ tools: ['Read', 'Grep'], platform: 'unknown', place: 'the build host' })
    expect(normalizeHostDeclaration({ tools: [] })).toEqual({
      tools: [],
      platform: 'unknown',
      place: 'the client host',
    })
  })

  it('compares declarations by content', () => {
    expect(sameHostDeclaration(undefined, undefined)).toBe(true)
    expect(sameHostDeclaration(linuxNoPwsh, undefined)).toBe(false)
    expect(sameHostDeclaration(linuxNoPwsh, declare(without('PowerShell'), 'linux', 'the workspace host'))).toBe(true)
    expect(sameHostDeclaration(linuxNoPwsh, windows)).toBe(false)
  })
})

describe('tools/list serves exactly the declared built-in tools', () => {
  it('keeps the full list and the legacy descriptions for a client without a declaration', async () => {
    expect(mcpToolsFor(undefined)).toBe(MCP_TOOLS)
    const tools = await listed(undefined)
    expect(builtin(tools)).toEqual(ALL)
    expect(tools.find((tool) => tool.name === 'PowerShell')!.description).toBe(
      MCP_TOOLS.find((tool) => tool.name === 'PowerShell')!.description,
    )
  })

  it('omits PowerShell for a Linux host without pwsh and names the declared place', async () => {
    const tools = await listed(linuxNoPwsh)
    expect(builtin(tools)).toEqual(without('PowerShell'))
    const text = JSON.stringify(tools)
    expect(LEGACY_WORDING.test(text)).toBe(false)
    expect(text.includes('PowerShell')).toBe(false)
    expect(tools.find((tool) => tool.name === 'Read')!.description).toContain('the filesystem of the workspace host')
  })

  it('serves a Windows host its declared set with the Windows PowerShell description', async () => {
    const tools = await listed(windows)
    expect(builtin(tools)).toEqual(without('preview_mount', 'preview_update', 'preview_unmount'))
    expect(tools.find((tool) => tool.name === 'PowerShell')!.description).toContain("on the user's machine (Windows)")
  })

  it('does not call a non-Windows host with pwsh a Windows host', async () => {
    const tools = await listed(declare(ALL, 'linux', 'the workspace host'))
    const powerShell = tools.find((tool) => tool.name === 'PowerShell')!.description
    expect(powerShell).toContain('on the workspace host (Linux)')
    expect(powerShell).toContain('This host is not Windows')
    expect(powerShell).not.toMatch(/registry.*are fine|Windows host/)
  })

  it('serves a reduced host without a shell only what it declared', async () => {
    const tools = await listed(noShell)
    expect(builtin(tools)).toEqual(['Read', 'Glob', 'Grep', 'AskUserQuestion'])
    expect(JSON.stringify(tools).includes('PowerShell')).toBe(false)
  })
})

describe('tools/call', () => {
  it('rejects an undeclared built-in tool with the reason and never forwards it', async () => {
    session.host = linuxNoPwsh
    const response = await request('tools/call', { name: 'PowerShell', arguments: { command: 'Get-Date' } })
    expect(response.status).toBe(400)
    expect((await response.json()).error).toMatchObject({
      code: -32602,
      message:
        'Tool PowerShell is not available in this session: the workspace host (Linux) did not declare it. Use one of the tools listed for this session.',
    })
    expect(mocks.call).not.toHaveBeenCalled()
  })

  it('still forwards a declared tool, and any built-in tool without a declaration', async () => {
    session.host = linuxNoPwsh
    expect((await request('tools/call', { name: 'Read', arguments: { file_path: '/p/a' } })).status).toBe(200)
    session.host = undefined
    expect((await request('tools/call', { name: 'PowerShell', arguments: { command: 'Get-Date' } })).status).toBe(200)
    expect(mocks.call).toHaveBeenCalledTimes(2)
  })
})

describe('instructions are worded from the declaration', () => {
  it('serves the declared host in initialize instructions', async () => {
    session.host = linuxNoPwsh
    const { instructions } = (await (await request('initialize')).json()).result
    expect(instructions).toBe(mcpInstructions(false, linuxNoPwsh))
    expect(instructions).toContain('user-tools file, shell and web tools run on the workspace host (Linux)')
    expect(instructions).toContain('Shell: Without MCP Tasks support, Bash run_in_background:true')
    expect(instructions).not.toMatch(/PowerShell|user's machine/)
  })

  it('drops shell, web and worktree guidance the host cannot use', () => {
    const text = mcpInstructions(true, noShell)
    expect(text).toContain('user-tools file tools run on the review host (Linux)')
    expect(text).not.toMatch(/Shell:|Bash|PowerShell|EnterWorktree/)
  })

  it('writes the session CLAUDE.md for the declared host', () => {
    const linux = bridgeClaudeMd(linuxNoPwsh)
    expect(linux).toContain('are on the workspace host (Linux), reached through Bridge')
    expect(linux).toContain('(Read, Write, Edit, Glob, Grep, Bash, WebFetch, ...) run on the workspace host.')
    expect(linux).toContain('Use the syntax of the host OS (Linux) and of the shell tool you call (Bash).')
    expect(linux).toContain('## Shell commands\nWith MCP Tasks support, Bash foreground calls')
    expect(linux).not.toMatch(LEGACY_WORDING)
    expect(linux).not.toContain('PowerShell')

    const win = bridgeClaudeMd(windows)
    expect(win).toContain("are on the user's machine (Windows), reached through Bridge")
    expect(win).toContain('(Windows) and of the shell tool you call (Bash and PowerShell)')

    const reduced = bridgeClaudeMd(noShell)
    expect(reduced).toContain('`mcp__user-tools__` file tools (Read, Glob, Grep, ...) run on the review host.')
    expect(reduced).toContain('This host declared no shell tool')
    expect(reduced).toContain('- Native WebSearch runs from the server.')
    expect(reduced).not.toMatch(/## Shell commands|EnterWorktree|\bBash\b|PowerShell/)
  })

  it('keeps the legacy CLAUDE.md, instructions and system prompt without a declaration', () => {
    expect(bridgeClaudeMd(undefined)).toBe(BRIDGE_DEFAULT_CLAUDE_MD)
    expect(BRIDGE_DEFAULT_CLAUDE_MD).toContain("Bash, PowerShell, WebFetch, ...) run on the user's machine.")
    expect(mcpInstructions(false)).toContain("user-tools file, shell and web tools run on the user's machine;")
    expect(buildSystemPrompt('/p', undefined, '/m')).toContain("on the user's machine, including")
    expect(buildSystemPrompt('/p', undefined, '/m')).toContain('(or Bash/PowerShell)')
  })

  it('names the declared place and shells in the transcript mirror prompt', () => {
    const linux = buildSystemPrompt('/p', undefined, '/m', linuxNoPwsh)
    expect(linux).toContain('mirrors conversation history on the workspace host, including')
    expect(linux).toContain('Read with mcp__user-tools__Read (or Bash).')
    expect(buildSystemPrompt('/p', undefined, '/m', noShell)).toContain('Read with mcp__user-tools__Read. The mirror')
  })
})
