// Same polyfill the entry point loads (the archive module pulls in the proxy).
import 'reflect-metadata'

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

// Engine switch at the conversation level: the plan reads only, the commit
// moves the conversation, and the files of every segment are archived,
// restored and deleted with it. Home and the data directory are temporary.

const holder = vi.hoisted(() => ({ dataDir: '' }))
vi.mock('../stats/database/lifecycle', () => ({ getDataDir: () => holder.dataDir }))

import {
  archiveConversation,
  commitSwitch,
  deleteConversationFiles,
  ensureSegmentTranscript,
  noteSegmentModel,
  planSwitch,
  restartSegmentFresh,
  segmentSubagentsDir,
  slugDirOf,
  takeTurnContext,
  unifiedLogPath,
} from './conversation'
import { activeSegment, loadManifest, resetManifestCacheForTests } from './manifest'
import { UnifiedLog } from './unified-log'
import { archivedTranscriptPath } from '../pty/transcript-archive'

const NATIVE = '11111111-2222-4333-8444-555555555555'
let home = ''
let envBefore: { HOME?: string; USERPROFILE?: string } = {}

beforeEach(() => {
  home = fs.mkdtempSync(path.join(os.tmpdir(), 'bridge-switch-'))
  holder.dataDir = path.join(home, 'data')
  envBefore = { HOME: process.env.HOME, USERPROFILE: process.env.USERPROFILE }
  process.env.HOME = home
  process.env.USERPROFILE = home
})

afterEach(() => {
  resetManifestCacheForTests()
  UnifiedLog.resetForTests()
  for (const k of ['HOME', 'USERPROFILE'] as const) {
    if (envBefore[k] === undefined) delete process.env[k]
    else process.env[k] = envBefore[k]
  }
  fs.rmSync(home, { recursive: true, force: true })
})

const line = (uuid: string, parentUuid: string | null, type: 'user' | 'assistant', text: string) =>
  JSON.stringify({ type, uuid, parentUuid, message: { role: type, content: text } })

/** A chat that ran on Claude: its settings dir and native transcript. */
function claudeChat() {
  const sessionsDir = path.join(home, '.claude', 'bridge-sessions', 'tok')
  const settingsDir = path.join(sessionsDir, 'chat-1')
  fs.mkdirSync(settingsDir, { recursive: true })
  const native = path.join(slugDirOf(settingsDir), `${NATIVE}.jsonl`)
  fs.mkdirSync(path.dirname(native), { recursive: true })
  fs.writeFileSync(
    native,
    [line('u1', null, 'user', 'The code word is FALCON.'), line('a1', 'u1', 'assistant', 'Noted.')].join('\n') + '\n',
  )
  return { sessionsDir, settingsDir, native }
}

function planFirst() {
  const chat = claudeChat()
  const plan = planSwitch({
    manifest: null,
    sessionsDir: chat.sessionsDir,
    from: { engine: 'claude', nativeId: NATIVE, settingsDir: chat.settingsDir, model: 'claude-haiku-5-5' },
    target: { engine: 'codex', model: 'gpt-6-luna' },
  })!
  return { chat, plan }
}

describe('engine switch: plan and commit', () => {
  it('plans without touching the conversation (B1: a failed start changes nothing)', () => {
    const { chat, plan } = planFirst()
    expect(plan.existing).toBe(false)
    expect(plan.segment).toMatchObject({ engine: 'codex', model: 'gpt-6-luna', nativeId: null })
    expect(plan.segment.pendingContext).toContain('The code word is FALCON.')
    expect(plan.segment.contextBytes).toBe(Buffer.byteLength(plan.segment.pendingContext!))
    // Nothing written: no manifest, no unified log, no compaction link target.
    expect(fs.existsSync(plan.ref.dir)).toBe(false)
    expect(fs.existsSync(unifiedLogPath(plan.ref))).toBe(false)
    expect(loadManifest(plan.ref.dir)).toBeNull()
    expect(fs.readFileSync(chat.native, 'utf8').split('\n').filter(Boolean)).toHaveLength(2)
  })

  it('an empty chat has nothing to carry', () => {
    const chat = claudeChat()
    expect(
      planSwitch({
        manifest: null,
        sessionsDir: chat.sessionsDir,
        from: { engine: 'claude', nativeId: null, settingsDir: chat.settingsDir, model: null },
        target: { engine: 'codex', model: null },
      }),
    ).toBeNull()
  })

  it('commit seeds the log, writes the marker and moves the manifest to the new segment', () => {
    const { plan } = planFirst()
    const manifest = commitSwitch(plan)
    expect(manifest.segments).toHaveLength(2)
    expect(manifest.active).toBe(1)
    expect(manifest.segments[0]).toMatchObject({ engine: 'claude', nativeId: NATIVE, logStart: 0 })
    const log = fs.readFileSync(unifiedLogPath(plan.ref), 'utf8').trim().split('\n')
    expect(log).toHaveLength(3)
    expect(JSON.parse(log[2]!)).toMatchObject({ subtype: 'engine_switch', parentUuid: 'a1', content: '→ gpt-6-luna' })
    expect(activeSegment(manifest).logStart).toBe(Buffer.byteLength(log.slice(0, 2).join('\n') + '\n'))
  })

  it('a later switch carries the whole conversation, both engines included', () => {
    const { chat, plan } = planFirst()
    const manifest = commitSwitch(plan)
    UnifiedLog.of(unifiedLogPath(plan.ref)).append([
      JSON.parse(line('c1', null, 'user', 'And the colour is teal.')),
      JSON.parse(line('c2', 'c1', 'assistant', 'Teal, noted.')),
    ])
    const back = planSwitch({
      manifest,
      sessionsDir: chat.sessionsDir,
      from: { engine: 'codex', nativeId: 'thread-1', settingsDir: plan.segment.settingsDir, model: 'gpt-6-luna' },
      target: { engine: 'claude', model: 'claude-haiku-5-5' },
    })!
    expect(back.existing).toBe(true)
    expect(back.segment.pendingContext).toContain('FALCON')
    expect(back.segment.pendingContext).toContain('teal')
    expect(back.segment.pendingContext).toContain('with\nClaude and ChatGPT')
  })
})

describe('engine switch: first-turn context', () => {
  it('is handed out once, with the first user turn', () => {
    const { plan } = planFirst()
    commitSwitch(plan)
    const first = takeTurnContext(plan.ref)
    expect(first).toContain('<bridge-context>')
    expect(first).toContain('FALCON')
    expect(takeTurnContext(plan.ref)).toBeNull()
    // Persisted: a restarted server does not send it again.
    resetManifestCacheForTests()
    expect(takeTurnContext(plan.ref)).toBeNull()
  })

  it('a segment whose native session is lost starts again with the whole conversation (S6)', () => {
    const { plan } = planFirst()
    const manifest = commitSwitch(plan)
    takeTurnContext(plan.ref)
    activeSegment(manifest).nativeId = 'thread-gone'
    const segment = restartSegmentFresh(manifest)
    expect(segment.nativeId).toBeNull()
    expect(segment.pump).toEqual({ file: null, pos: 0 })
    expect(takeTurnContext(plan.ref)).toContain('FALCON')
  })

  it('records an in-place model change on the running segment', () => {
    const { plan } = planFirst()
    commitSwitch(plan)
    noteSegmentModel(plan.ref, 'gpt-6-sol')
    resetManifestCacheForTests()
    expect(activeSegment(loadManifest(plan.ref.dir)!).model).toBe('gpt-6-sol')
  })
})

describe('engine switch: segment files', () => {
  it('archives every segment and restores a swept native transcript (S6, INC-2026-0054)', async () => {
    const { chat, plan } = planFirst()
    commitSwitch(plan)
    await archiveConversation(plan.ref)
    expect(archivedTranscriptPath(NATIVE)).not.toBeNull()
    expect(fs.existsSync(archivedTranscriptPath(plan.ref.id)!)).toBe(true)
    expect(fs.existsSync(archivedTranscriptPath(NATIVE)!)).toBe(true)

    fs.rmSync(chat.native) // the CLI's cleanup swept it
    const first = loadManifest(plan.ref.dir)!.segments[0]!
    expect(await ensureSegmentTranscript(first)).toBe(true)
    expect(fs.readFileSync(chat.native, 'utf8')).toContain('FALCON')
  })

  it('finds sub-agent transcripts under the running segment, not the logical id (S9)', () => {
    const { plan } = planFirst()
    const manifest = commitSwitch(plan)
    expect(segmentSubagentsDir(plan.ref)).toBeNull() // the new segment has no native id yet
    activeSegment(manifest).nativeId = 'native-b'
    expect(segmentSubagentsDir(plan.ref)).toBe(path.join(slugDirOf(plan.segment.settingsDir), 'native-b', 'subagents'))
  })

  it('deletion removes every segment directory, native transcript, archive and the cache (S7)', async () => {
    const { chat, plan } = planFirst()
    const manifest = commitSwitch(plan)
    fs.mkdirSync(plan.segment.settingsDir, { recursive: true })
    const nativeB = path.join(slugDirOf(plan.segment.settingsDir), 'native-b.jsonl')
    fs.mkdirSync(path.dirname(nativeB), { recursive: true })
    fs.writeFileSync(nativeB, line('b1', null, 'user', 'x') + '\n')
    fs.mkdirSync(path.join(slugDirOf(plan.segment.settingsDir), 'native-b', 'subagents'), { recursive: true })
    activeSegment(manifest).nativeId = 'native-b'
    await archiveConversation(plan.ref)

    const owned = (dir: string) => dir.startsWith(chat.sessionsDir)
    deleteConversationFiles(plan.ref.dir, owned)
    expect(fs.existsSync(chat.native)).toBe(false)
    expect(fs.existsSync(unifiedLogPath(plan.ref))).toBe(false)
    expect(fs.existsSync(nativeB)).toBe(false)
    expect(fs.existsSync(path.join(slugDirOf(plan.segment.settingsDir), 'native-b'))).toBe(false)
    expect(fs.existsSync(plan.segment.settingsDir)).toBe(false)
    expect(fs.existsSync(archivedTranscriptPath(NATIVE)!)).toBe(false)
    expect(fs.existsSync(archivedTranscriptPath(plan.ref.id)!)).toBe(false)
    // The chat's original directory is the first segment's: it goes too.
    expect(fs.existsSync(chat.settingsDir)).toBe(false)
    fs.rmSync(path.join(plan.ref.dir, 'conversation.json'))
    expect(loadManifest(plan.ref.dir)).toBeNull()
  })

  it('deletion never removes a directory it does not own', () => {
    const { plan } = planFirst()
    commitSwitch(plan)
    fs.mkdirSync(plan.segment.settingsDir, { recursive: true })
    deleteConversationFiles(plan.ref.dir, () => false)
    expect(fs.existsSync(plan.segment.settingsDir)).toBe(true)
  })
})
