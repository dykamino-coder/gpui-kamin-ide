import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

/**
 * INC-2026-0070: the durable lifecycle journal must show, per created
 * session, whether a resume was requested, for which (pseudonymous)
 * conversation, whether the lookup found it, and every conversation id the
 * client is later told to adopt (old→new). Exercised through the actual
 * createSession path; only the PTY, filesystem lookup and watcher are faked.
 */

const h = vi.hoisted(() => ({
  spawn: vi.fn(() => ({ pid: 1, onExit: () => ({ dispose() {} }), kill() {} })),
  root: '',
  found: null as string | null,
  onConversationId: null as null | ((id: string) => void),
}))
vi.mock('node-pty', () => ({
  spawn: h.spawn,
}))
vi.mock('./session-settings', () => ({
  get SESSIONS_BASE() {
    return h.root
  },
  sanitizeDirName: () => 'cwd',
  writeSessionSettings: vi.fn(),
  writeSessionClaudeMd: vi.fn(),
  applySyncData: vi.fn(),
}))
vi.mock('./session-env', () => ({ buildSessionEnv: () => ({}), modelForResume: (s: string) => s }))
vi.mock('./session-plugin-args', () => ({ buildSessionClaudeArgs: () => [] }))
vi.mock('./session-resume-helpers', () => ({
  findOrRecreateSettingsDir: () => h.found,
  xbasename: (s: string) => path.basename(s),
  repairTranscriptForResume: vi.fn(),
  resolveNewestInChain: () => null,
  lastModelEntryForResume: () => null,
}))
vi.mock('./jsonl-watcher', () => ({
  JsonlWatcher: class {
    constructor(...args: unknown[]) {
      h.onConversationId = args[5] as (id: string) => void
    }
    start = vi.fn()
    stop = vi.fn()
  },
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
vi.mock('../proxy/native-mitm', () => ({ startNativeMitm: vi.fn() }))
vi.mock('../proxy/streaming-settings', () => ({ getStreamingSettings: async () => ({ enabled: false }) }))
vi.mock('../logging', () => ({ debugLog: vi.fn(), infoLog: vi.fn(), warnLog: vi.fn(), errorLog: vi.fn() }))
vi.mock('./transcript-archive', () => ({ archiveTranscriptForSession: vi.fn(), dropArchivedTranscript: vi.fn() }))

const A = '00000000-0000-4000-8000-000000000001'
const B = '00000000-0000-4000-8000-000000000002'
const socket = { readyState: 1, send: vi.fn() } as any
let core: typeof import('./session-core')
let journal = ''

function rows(): Array<Record<string, unknown>> {
  const file = path.join(journal, 'lifecycle.jsonl')
  if (!fs.existsSync(file)) return []
  return fs
    .readFileSync(file, 'utf8')
    .trim()
    .split('\n')
    .map((line) => JSON.parse(line))
}

beforeEach(async () => {
  vi.resetModules()
  h.root = fs.mkdtempSync(path.join(os.tmpdir(), 'identity-test-'))
  journal = path.join(h.root, 'journal')
  vi.stubEnv('BRIDGE_LIFECYCLE_LOG_DIR', journal)
  // The compact-link map lives under the home directory and is resolved on
  // import: keep the changed-id case from touching the real ~/.claude.
  vi.stubEnv('HOME', h.root)
  vi.stubEnv('USERPROFILE', h.root)
  h.spawn.mockClear()
  h.found = null
  h.onConversationId = null
  core = await import('./session-core')
  expect(os.homedir()).toBe(h.root)
})

afterEach(() => {
  core.sessions.clear()
  vi.unstubAllEnvs()
  if (path.basename(h.root).startsWith('identity-test-')) fs.rmSync(h.root, { recursive: true, force: true })
})

describe('INC-2026-0070 identity lifecycle', () => {
  it('INC-0074 opted-in not-found creates no PTY or session; explicit create then spawns', async () => {
    const { ResumeNotFoundError } = await import('./session-resume-not-found')
    await expect(
      core.createSession(socket, 'tester', 'tok', {
        resumeConversationId: A,
        noSpawnOnNotFound: true,
      }),
    ).rejects.toBeInstanceOf(ResumeNotFoundError)
    expect(h.spawn).not.toHaveBeenCalled()
    expect(h.onConversationId).toBeNull()
    expect(core.sessions.size).toBe(0)
    expect(rows().some((r) => r.event === 'session_create')).toBe(false)
    await core.createSession(socket, 'tester', 'tok', {})
    expect(h.spawn).toHaveBeenCalledTimes(1)
    expect(core.sessions.size).toBe(1)
  })

  it('a resume that finds the transcript records the request, the lookup and the confirmed id', async () => {
    h.found = path.join(h.root, 'tok-cwd', 'old-session')
    await core.createSession(socket, 'tester', 'tok', { resumeConversationId: A })
    h.onConversationId!(A)

    const created = rows().find((r) => r.event === 'session_create')!
    expect(created).toMatchObject({
      resumeRequested: true,
      resumeNotFound: false,
      settingsDirFound: true,
      settingsDirSource: 'found',
    })
    expect(created.resumeRef).toMatch(/^[a-f0-9]{12}$/)
    const confirmed = rows().find((r) => r.event === 'conversation_id')!
    expect(confirmed).toMatchObject({ idSource: 'discovered', conversationRef: created.resumeRef })
    expect(confirmed.previousRef).toBe(created.resumeRef)
  })

  it('a resume of a missing transcript records not-found and the fresh id the client is offered', async () => {
    await core.createSession(socket, 'tester', 'tok', { resumeConversationId: A })
    h.onConversationId!(B)

    const created = rows().find((r) => r.event === 'session_create')!
    expect(created).toMatchObject({
      resumeRequested: true,
      resumeNotFound: true,
      settingsDirFound: false,
      settingsDirSource: 'fresh_not_found',
    })
    const adopted = rows().find((r) => r.event === 'conversation_id')!
    expect(adopted.idSource).toBe('discovered')
    expect(adopted).not.toHaveProperty('previousRef')
    expect(adopted.conversationRef).not.toBe(created.resumeRef)
  })

  it('a create without a resume id is distinguishable, and a changed id logs old→new', async () => {
    await core.createSession(socket, 'tester', 'tok', {})
    h.onConversationId!(A)
    h.onConversationId!(B)

    expect(rows().find((r) => r.event === 'session_create')).toMatchObject({
      resumeRequested: false,
      settingsDirSource: 'fresh',
    })
    const ids = rows().filter((r) => r.event === 'conversation_id')
    expect(ids.map((r) => r.idSource)).toEqual(['discovered', 'changed'])
    expect(ids[1]!.previousRef).toBe(ids[0]!.conversationRef)
    const content = fs.readFileSync(path.join(journal, 'lifecycle.jsonl'), 'utf8')
    expect(content).not.toContain(A)
    expect(content).not.toContain(B)
  })
})
