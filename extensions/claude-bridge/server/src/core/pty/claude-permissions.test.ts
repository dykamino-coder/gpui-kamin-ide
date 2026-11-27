import 'reflect-metadata'
import { afterEach, beforeEach, describe, it, expect, vi } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import {
  observeClaudePermission,
  permissionChangeBlocker,
  publishClaudePermissions,
  pristineClaudeChatId,
  restoredClaudeMode,
  retainClaudeMode,
} from './claude-permissions'
import { SessionCliControl } from './session-cli-control'
import type { PtySession } from './types'
import * as conversations from '../conversation/conversation'
import * as database from '../stats/database/lifecycle'

let dir: string
beforeEach(() => {
  dir = fs.mkdtempSync(path.join(os.tmpdir(), 'permission-fixture-'))
  vi.spyOn(database, 'getDataDir').mockReturnValue(dir)
})
afterEach(() => {
  vi.restoreAllMocks()
  fs.rmSync(dir, { recursive: true, force: true })
})

function fixture(id = 'chat-one', owner = 'owner-one') {
  const file = path.join(dir, id + '.jsonl')
  const frames: any[] = []
  const session = {
    id,
    tokenId: owner,
    cliConversationId: id,
    settingsDir: path.join(dir, id),
    state: 'running',
    lastActivityAt: new Date(0),
    childSessions: [],
    pendingElicitations: new Map(),
    jsonlWatcher: { getFilePath: () => file },
    pty: { write: vi.fn() },
    ws: { readyState: 1, bufferedAmount: 0, send: (v: string) => frames.push(JSON.parse(v)) },
  } as unknown as PtySession
  const control = new SessionCliControl(session)
  const status = (value: string) => control.parser.feed('\x1b]7501;state=' + value + ':app=claude-code\x07')
  const hook = (permission_mode: string, session_id = id, agent_id?: string) =>
    observeClaudePermission(session, { permission_mode, session_id, agent_id, hook_event_name: 'Stop' })
  const record = (type = 'assistant', timestamp = new Date().toISOString()) =>
    fs.writeFileSync(
      file,
      JSON.stringify({
        type,
        timestamp,
        message: { stop_reason: 'end_turn', content: [{ type: 'text', text: 'fixture' }] },
      }) + '\n',
    )
  return { session, control, frames, hook, file, status, record }
}

describe('native Claude permission selection', () => {
  it('keeps requested distinct from the main CLI observation, including native-only modes', () => {
    const a = fixture()
    a.session.launchConfig = { claudePermissionMode: 'plan' }
    publishClaudePermissions(a.session)
    expect(a.frames.at(-1)).toMatchObject({ available: [], requested: 'plan' })
    expect(a.frames.at(-1)).not.toHaveProperty('selected')
    a.hook('auto', 'foreign')
    a.hook('auto', a.session.id, 'child')
    a.hook('unknown')
    expect(a.frames).toHaveLength(1)
    a.hook('bypassPermissions')
    expect(a.frames.at(-1)).toMatchObject({
      selected: 'bypassPermissions',
      available: ['default', 'acceptEdits', 'plan'],
      requested: 'plan',
    })
    a.hook('plan')
    expect(a.frames.at(-1).selected).toBe('plan')
  })
  it('requires a fresh clean native prompt and a recorded response for this chat', () => {
    const a = fixture()
    a.record()
    a.status('idle')
    expect(permissionChangeBlocker(a.session)).toContain('runtime')
    a.hook('default')
    expect(permissionChangeBlocker(a.session)).toBeUndefined()
    for (const status of ['working', 'blocked', 'clear', 'error']) {
      a.status(status)
      expect(permissionChangeBlocker(a.session)).toContain('idle')
    }
    a.status('done')
    a.control.rawInput('\r')
    expect(permissionChangeBlocker(a.session)).toContain('idle')
    a.status('idle')
    a.record('assistant', new Date(0).toISOString())
    expect(permissionChangeBlocker(a.session)).toContain('saved')
    a.record('user')
    expect(permissionChangeBlocker(a.session)).toContain('saved')
    fs.writeFileSync(a.file, '{"type":"assistant"')
    expect(permissionChangeBlocker(a.session)).toContain('saved')
    a.record()
    a.session.cliConversationId = 'different'
    expect(permissionChangeBlocker(a.session)).toContain('runtime')
    a.session.cliConversationId = a.session.id
    a.session.pendingElicitations.set('pending', {} as never)
    expect(permissionChangeBlocker(a.session)).toContain('idle')
    a.session.pendingElicitations.clear()
    expect(permissionChangeBlocker(a.session)).toBeUndefined()
  })
  it('allows a confirmed untouched native prompt before its first turn and refuses a delayed transcript or user input', () => {
    const id = '12345678-1234-1234-1234-123456789abc'
    const a = fixture(id)
    a.session.cliConversationId = null
    a.session.inputCount = 0
    a.session.mcpCallCount = 0
    a.session.jsonlWatcher = { getFilePath: () => null } as any
    vi.spyOn(conversations, 'slugDirOf').mockReturnValue(dir)
    const startup = () =>
      observeClaudePermission(a.session, {
        hook_event_name: 'SessionStart',
        source: 'startup',
        session_id: id,
      })
    startup()
    a.status('idle')
    expect(pristineClaudeChatId(a.session)).toBe(id)
    expect(permissionChangeBlocker(a.session)).toBeUndefined()
    a.record()
    expect(pristineClaudeChatId(a.session)).toBeUndefined()
    fs.unlinkSync(a.file)
    observeClaudePermission(a.session, { hook_event_name: 'Notification', session_id: id })
    expect(pristineClaudeChatId(a.session)).toBeUndefined()
    startup()
    a.control.rawInput('x')
    expect(pristineClaudeChatId(a.session)).toBeUndefined()
    expect(permissionChangeBlocker(a.session)).toBeTruthy()
  })
  it('preserves an explicit choice by owner and chat across a cold resume or directory replacement', () => {
    const a = fixture('first')
    const b = fixture('second')
    retainClaudeMode(a.session, 'plan')
    retainClaudeMode(b.session, 'acceptEdits')
    const canonical = (id: string) => (id === 'first' ? 'compacted' : id)
    expect(restoredClaudeMode(a.session.tokenId, 'first', 'new-dir', canonical)).toBe('plan')
    expect(restoredClaudeMode(a.session.tokenId, 'compacted', a.session.settingsDir, canonical)).toBe('plan')
    expect(restoredClaudeMode(a.session.tokenId, 'second', b.session.settingsDir, canonical)).toBe('acceptEdits')
    expect(restoredClaudeMode('other', 'first', a.session.settingsDir, canonical)).toBeUndefined()
    expect(restoredClaudeMode(a.session.tokenId, 'unrelated', a.session.settingsDir, canonical)).toBeUndefined()
    const store = path.join(dir, 'claude-permission-selections')
    fs.writeFileSync(path.join(store, fs.readdirSync(store)[0]!), '{')
    expect(() => restoredClaudeMode(a.session.tokenId, 'first', a.session.settingsDir, canonical)).toThrow()
  })
})
