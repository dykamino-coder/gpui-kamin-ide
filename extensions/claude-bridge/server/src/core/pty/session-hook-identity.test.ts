// Настройки беседы общие для resume, но адреса hooks принадлежат одному CLI.
import 'reflect-metadata'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const h = vi.hoisted(() => ({
  root: '',
  sync: '',
  spawn: vi.fn((_cmd: string, _args: string[], _options: { env: Record<string, string> }) => ({
    pid: 1,
    onExit: vi.fn((_callback: (event: { exitCode: number }) => void) => ({ dispose() {} })),
    kill() {},
  })),
  streaming: vi.fn(async () => ({ enabled: false })),
}))
vi.mock('node-pty', () => ({ spawn: h.spawn }))
vi.mock('../sync/routes', () => ({
  getUserSyncDir: () => h.sync,
  getProjectSyncDir: () => h.sync,
}))
vi.mock('./mcp-tasks', () => ({
  mcpIdentity: async () => ({ id: 'durable-endpoint', token: 'durable-credential' }),
  startMcpTaskDelivery: vi.fn(),
  settleMcpTask: vi.fn(async () => false),
  mcpTasks: { hasWorkingOwner: () => false },
}))
vi.mock('./session-resume-helpers', () => ({
  findOrRecreateSettingsDir: () => path.join(h.root, 'conversation'),
  xbasename: (s: string) => path.basename(s),
  repairTranscriptForResume: vi.fn(),
  resolveNewestInChain: () => null,
  lastModelEntryForResume: () => null,
}))
vi.mock('./jsonl-watcher', () => ({
  JsonlWatcher: class {
    start = vi.fn()
    stop = vi.fn()
  },
}))
vi.mock('./session-statusline', () => ({
  writeStatusLineScript: () => ({ type: 'command', command: 'true' }),
  watchSessionStatusLine: vi.fn(),
}))
vi.mock('./session-io', () => ({
  sendToClient: vi.fn(),
  writeInputToSession: vi.fn(),
  submitTextToSession: vi.fn(),
  resizeSessionTerminal: vi.fn(),
  attachStartupAutoResponder: vi.fn(),
  attachOutputDebounce: vi.fn(),
  clearInputThrottle: vi.fn(),
}))
vi.mock('../proxy/streaming-settings', () => ({ getStreamingSettings: h.streaming }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), infoLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))
vi.mock('./transcript-archive', () => ({ archiveTranscriptForSession: vi.fn(), dropArchivedTranscript: vi.fn() }))
vi.mock('../hooks/dispatcher', () => ({
  dispatchHook: vi.fn(async () => ({ stdout: '', stderr: '', exitCode: 0, outcome: 'success', durationMs: 1 })),
}))

let core: typeof import('./session-core')
let clearHooks: typeof import('../hooks/registry').clearSession
let app: ReturnType<typeof import('../hooks/routes').createHooksRoutes>
const socket = { readyState: 1, send: vi.fn() } as any
const conversation = '00000000-0000-4000-8000-000000000001'

beforeEach(async () => {
  vi.resetModules()
  h.root = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-hook-identity-'))
  h.sync = path.join(h.root, 'sync')
  vi.stubEnv('HOME', h.root)
  vi.stubEnv('USERPROFILE', h.root)
  vi.stubEnv('BRIDGE_LIFECYCLE_LOG_DIR', path.join(h.root, 'journal'))
  h.spawn.mockClear()
  h.streaming.mockReset().mockResolvedValue({ enabled: false })
  fs.mkdirSync(h.sync)
  fs.writeFileSync(
    path.join(h.sync, 'settings.json'),
    JSON.stringify({
      hooks: {
        PreToolUse: [{ matcher: 'mcp__user-tools__Grep', hooks: [{ type: 'command', command: 'guard' }] }],
        PostToolUse: [{ matcher: 'mcp__user-tools__Grep', hooks: [{ type: 'command', command: 'notify' }] }],
      },
    }),
  )
  core = await import('./session-core')
  clearHooks = (await import('../hooks/registry')).clearSession
  app = (await import('../hooks/routes')).createHooksRoutes()
})

afterEach(() => {
  for (const id of core.sessions.keys()) clearHooks(id)
  core.sessions.clear()
  vi.unstubAllEnvs()
  fs.rmSync(h.root, { recursive: true, force: true })
})

function launchHooks(call: (typeof h.spawn.mock.calls)[number]) {
  const args = call[1]
  const at = args.indexOf('--settings')
  // До исправления CLI читает перезаписанный файл только после spawn.
  const settings =
    at < 0
      ? JSON.parse(fs.readFileSync(path.join(h.root, 'conversation', '.claude', 'settings.json'), 'utf8'))
      : JSON.parse(fs.readFileSync(args[at + 1]!, 'utf8'))
  return settings.hooks as Record<string, Array<{ hooks: Array<{ args: string[] }> }>>
}

async function hookStatus(call: (typeof h.spawn.mock.calls)[number], event: string, token?: string) {
  const relay = launchHooks(call)[event]![0]!.hooks[0]!.args[1]!
  const url = relay.match(/request\(("[^" ]+")/)![1]!
  return (
    await app.request(
      new Request(JSON.parse(url), {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'X-Bridge-Hook-Relay': 'command',
          Authorization: `Bearer ${token ?? call[2].env.CLAUDE_BRIDGE_HOOK_TOKEN}`,
        },
        body: JSON.stringify({ hook_event_name: event, tool_name: 'mcp__user-tools__Grep' }),
      }),
    )
  ).status
}

it('keeps overlapping resumes bound to their own session across shared settings rewrites', async () => {
  let release!: () => void
  let reached!: () => void
  const paused = new Promise<void>((resolve) => {
    reached = resolve
  })
  const gate = new Promise<void>((resolve) => {
    release = resolve
  })
  h.streaming.mockImplementationOnce(async () => {
    reached()
    await gate
    return { enabled: false }
  })
  const first = core.createSession(socket, 'synthetic', 'owner', {
    resumeConversationId: conversation,
    bearerHash: 'owner',
  })
  await paused
  try {
    await core.createSession(socket, 'synthetic', 'owner', { resumeConversationId: conversation, bearerHash: 'owner' })
  } finally {
    release()
  }
  await first
  expect(h.spawn).toHaveBeenCalledTimes(2)
  for (const call of h.spawn.mock.calls) {
    const other = h.spawn.mock.calls.find((candidate) => candidate !== call)!
    for (const event of ['PreToolUse', 'PostToolUse']) {
      expect(await hookStatus(call, event)).toBe(200)
      expect(await hookStatus(call, event, 'wrong-credential')).toBe(401)
      expect(await hookStatus(call, event, other[2].env.CLAUDE_BRIDGE_HOOK_TOKEN)).toBe(401)
    }
    expect(JSON.stringify(launchHooks(call))).not.toContain(call[2].env.CLAUDE_BRIDGE_HOOK_TOKEN)
  }
})

it('keeps a running CLI bound to its hooks after a later resume rewrites persisted settings', async () => {
  const projectFile = path.join(h.root, 'conversation', '.claude', 'settings.json')
  fs.mkdirSync(path.dirname(projectFile), { recursive: true })
  fs.writeFileSync(
    projectFile,
    JSON.stringify({ hooks: { SessionStart: [{ hooks: [{ type: 'command', command: 'stale-relay' }] }] } }),
  )
  const config = { resumeConversationId: conversation, bearerHash: 'owner' }
  await core.createSession(socket, 'synthetic', 'owner', config)
  const firstLaunch = h.spawn.mock.calls[0]!
  await core.createSession(socket, 'synthetic', 'owner', config)
  expect(await hookStatus(firstLaunch, 'PreToolUse')).toBe(200)
  const project = JSON.parse(fs.readFileSync(projectFile, 'utf8'))
  expect(project).not.toHaveProperty('hooks')
  expect(project.permissions.deny).toContain('Grep')
  expect(project.statusLine).toEqual({ type: 'command', command: 'true' })
})

it('removes only the exiting CLI settings file, preserving the other resumed process', async () => {
  const config = { resumeConversationId: conversation, bearerHash: 'owner' }
  await core.createSession(socket, 'synthetic', 'owner', config)
  await core.createSession(socket, 'synthetic', 'owner', config)
  const first = h.spawn.mock.calls[0]!
  const second = h.spawn.mock.calls[1]!
  const firstFile = first[1][first[1].indexOf('--settings') + 1]!
  const secondFile = second[1][second[1].indexOf('--settings') + 1]!
  expect(firstFile).not.toBe(secondFile)
  h.spawn.mock.results[0]!.value.onExit.mock.calls[0]![0]({ exitCode: 0 })
  expect(fs.existsSync(firstFile)).toBe(false)
  expect(fs.existsSync(secondFile)).toBe(true)
  expect(await hookStatus(second, 'PreToolUse')).toBe(200)
})

it('removes a failed launch snapshot without touching another CLI in the reused directory', async () => {
  const settingsDir = path.join(h.root, 'conversation')
  const config = { reuseSettingsDir: settingsDir, bearerHash: 'owner' }
  await core.createSession(socket, 'synthetic', 'owner', config)
  const before = fs.readdirSync(settingsDir).filter((name) => name.startsWith('.bridge-hooks-'))
  h.spawn.mockImplementationOnce(() => {
    throw new Error('synthetic spawn failure')
  })
  await expect(core.createSession(socket, 'synthetic', 'owner', config)).rejects.toThrow('synthetic spawn failure')
  expect(fs.readdirSync(settingsDir).filter((name) => name.startsWith('.bridge-hooks-'))).toEqual(before)
  expect(await hookStatus(h.spawn.mock.calls[0]!, 'PreToolUse')).toBe(200)
})

it('passes large native hook declarations through a file instead of an oversized argv value', async () => {
  fs.writeFileSync(
    path.join(h.sync, 'settings.json'),
    JSON.stringify({ hooks: { Stop: [{ hooks: [{ type: 'prompt', prompt: 'x'.repeat(150_000) }] }] } }),
  )
  await core.createSession(socket, 'synthetic', 'owner', { resumeConversationId: conversation, bearerHash: 'owner' })
  const call = h.spawn.mock.calls[0]!
  const file = call[1][call[1].indexOf('--settings') + 1]!
  expect(fs.statSync(file).size).toBeGreaterThan(150_000)
  expect(call[1].every((arg) => arg.length < 4096)).toBe(true)
})

it('always acknowledges Bridge Stop and SubagentStop status hooks during an active stop hook', async () => {
  await core.createSession(socket, 'synthetic', 'owner', { resumeConversationId: conversation, bearerHash: 'owner' })
  const call = h.spawn.mock.calls[0]!
  for (const event of ['Stop', 'SubagentStop']) {
    const relay = launchHooks(call)[event]![0]!.hooks[0]!.args[1]!
    const url = JSON.parse(relay.match(/request\(("[^" ]+")/)![1]!)
    for (const commandRelay of [true, false]) {
      const response = await app.request(
        new Request(url, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${call[2].env.CLAUDE_BRIDGE_HOOK_TOKEN}`,
            ...(commandRelay ? { 'X-Bridge-Hook-Relay': 'command' } : {}),
          },
          body: JSON.stringify({ hook_event_name: event, stop_hook_active: true }),
        }),
      )
      expect(response.status).toBe(200)
      expect(await response.json()).toEqual(
        commandRelay
          ? { result: { stdout: '', stderr: '', exitCode: 0, outcome: 'success', durationMs: 0 } }
          : { continue: true, suppressOutput: true },
      )
    }
  }
})
