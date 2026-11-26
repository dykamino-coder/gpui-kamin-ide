// The interactive CLI owns its permission mode. Report only an observation
// from this terminal, and retain explicit launch choices across a cold resume.
import fs from 'node:fs'
import path from 'node:path'
import { createHash, randomUUID } from 'node:crypto'
import { getDataDir } from '../stats/database/lifecycle'
import type { PtySession } from './types'
import { getSessionInputSnapshot } from './session-input-coordinator'
import { sessionCliControl } from './session-cli-control'
import { hibernationBlocker } from './session-hibernation'
import { sendToClient } from './session-io'
import { activeSegment, loadManifest } from '../conversation/manifest'
import { nativeTranscriptPath } from '../conversation/conversation'
import type { HookInputPayload } from '../hooks/types'

export const selectableClaudeModes = ['default', 'acceptEdits', 'plan'] as const
export type ClaudeMode = (typeof selectableClaudeModes)[number]
export function isClaudeMode(mode: unknown): mode is ClaudeMode {
  return selectableClaudeModes.some((candidate) => candidate === mode)
}
const observed = new WeakMap<PtySession, { mode: string; id: string }>()
export function observedClaudeMode(session: PtySession): string | undefined {
  const receipt = observed.get(session)
  const id = session.conversation ? currentSegment(session)?.nativeId : session.cliConversationId
  return receipt && (!id || receipt.id === id) ? receipt.mode : undefined
}
export function publishClaudePermissions(session: PtySession, reason?: string) {
  if (session.engine === 'codex') return
  const selected = observedClaudeMode(session)
  const supported = selected !== undefined || sessionCliControl(session)?.pinnedModReady() === true
  sendToClient(session.ws, {
    type: 'session:claude-permissions',
    sessionId: session.id,
    available: supported ? selectableClaudeModes : [],
    selected,
    requested: session.launchConfig?.claudePermissionMode,
    reason: reason ?? (supported ? undefined : 'Open the native Console to see the available Claude modes.'),
  })
}

export function observeClaudePermission(session: PtySession, payload: HookInputPayload): void {
  const mode = payload.permission_mode
  if (
    session.engine === 'codex' ||
    session.teardown ||
    session.isSubAgent ||
    payload.agent_id ||
    typeof mode !== 'string' ||
    !['default', 'acceptEdits', 'plan', 'auto', 'dontAsk', 'bypassPermissions'].includes(mode) ||
    (observed.get(session)?.id === payload.session_id && observed.get(session)?.mode === mode)
  )
    return
  const nativeId = session.conversation ? currentSegment(session)?.nativeId : session.cliConversationId
  if (!payload.session_id || (nativeId && payload.session_id !== nativeId)) return
  observed.set(session, { mode, id: payload.session_id })
  publishClaudePermissions(session)
}
const digest = (value: string) => createHash('sha256').update(value).digest('hex')

function ownerStore(owner: string) {
  const file = path.join(getDataDir(), 'claude-permission-selections', digest(owner) + '.json')
  let values: Record<string, { id: string; mode: ClaudeMode }>
  try {
    values = JSON.parse(fs.readFileSync(file, 'utf8'))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    values = {}
  }
  if (!values || typeof values !== 'object' || Array.isArray(values))
    throw new Error('The saved Claude permission choices are unreadable.')
  return { file, values }
}
export function restoredClaudeMode(
  owner: string,
  id: string | undefined,
  directory: string,
  canonical: (id: string) => string,
): ClaudeMode | undefined {
  if (!id) return
  const { values } = ownerStore(owner)
  const keyed = values[digest(id)]
  // An inline /resume can share a directory between distinct conversations.
  // Only a confirmed compact chain lets the directory receipt identify its successor.
  const entry = keyed ?? values[digest(directory)]
  if (!entry) return
  if (typeof entry.id !== 'string' || !isClaudeMode(entry.mode))
    throw new Error('The saved Claude permission mode is unreadable.')
  if (keyed || canonical(entry.id) === id) return entry.mode
}

export function retainClaudeMode(session: PtySession, mode: ClaudeMode, alias?: string): void {
  const { file, values } = ownerStore(session.tokenId)
  const id = session.conversation?.id ?? session.cliConversationId
  if (!id) throw new Error('Claude has not reported the conversation identity.')
  // One atomic write covers the current ID, compaction and multi-engine handoff.
  for (const key of [session.settingsDir, session.cliConversationId, id, alias])
    if (key) values[digest(key)] = { id, mode }
  writeSelections(file, values)
}

function writeSelections(file: string, values: ReturnType<typeof ownerStore>['values']) {
  fs.mkdirSync(path.dirname(file), { recursive: true })
  const tmp = file + '.' + randomUUID() + '.tmp'
  try {
    fs.writeFileSync(tmp, JSON.stringify(values), { mode: 0o600 })
    fs.renameSync(tmp, file)
  } finally {
    fs.rmSync(tmp, { force: true })
  }
}

export function forgetClaudeMode(owner: string, id: string): void {
  const { file, values } = ownerStore(owner)
  const group = values[digest(id)]?.id ?? id
  const keys = Object.keys(values).filter((k) => k === digest(id) || values[k]?.id === group)
  if (!keys.length) return
  for (const key of keys) delete values[key]
  writeSelections(file, values)
}

function currentSegment(session: PtySession) {
  const manifest = session.conversation && loadManifest(session.conversation.dir)
  const segment = manifest && activeSegment(manifest)
  return segment?.engine === 'claude' ? segment : undefined
}

export function permissionChangeBlocker(session: PtySession): string | undefined {
  if (!observedClaudeMode(session) && !sessionCliControl(session)?.pinnedModReady())
    return 'The active Claude runtime has not reported a permission mode. Use the native Console.'
  const idleSince = sessionCliControl(session)?.safeRestartSince()
  const input = getSessionInputSnapshot(session)
  const blocked = hibernationBlocker(session, Date.now(), 0)
  if (
    blocked ||
    !input.promptReady ||
    !input.promptObserved ||
    input.rawInputDirty ||
    input.activeKind ||
    input.queuedSubmissions ||
    input.pendingMaintenance.length ||
    idleSince === undefined
  )
    return 'Claude must be idle at an empty prompt. Finish or dismiss any pending request in the native Console.'
  const segment = currentSegment(session)
  const nativeId = session.conversation ? segment?.nativeId : session.cliConversationId
  const transcript = segment ? nativeTranscriptPath(segment) : session.jsonlWatcher?.getFilePath()
  if (
    !nativeId ||
    !transcript ||
    path.basename(transcript) !== nativeId + '.jsonl' ||
    !hasRecordedReply(transcript, idleSince)
  )
    return 'A completed turn must be saved before this chat can resume with another permission mode.'
}

function hasRecordedReply(file: string, submittedAt: number): boolean {
  let fd: number | undefined
  try {
    fd = fs.openSync(file, 'r')
    const size = fs.fstatSync(fd).size
    const start = Math.max(0, size - 128 * 1024)
    const bytes = Buffer.alloc(size - start)
    fs.readSync(fd, bytes, 0, bytes.length, start)
    const rows = bytes.toString('utf8').split('\n')
    if (rows.pop() !== '') return false
    if (start) rows.shift()
    let ended = false
    for (const line of rows.reverse()) {
      if (!line) continue
      const row = JSON.parse(line)
      if (row.isSidechain) continue
      if (row.type === 'system' && row.subtype === 'turn_duration') ended = true
      if (row.type === 'user') return false
      if (row.type === 'assistant' && submittedAt && !(Date.parse(row.timestamp) >= submittedAt)) return false
      if (row.type === 'assistant')
        return (
          ['end_turn', 'stop_sequence'].includes(row.message?.stop_reason) ||
          (ended && row.message?.content?.some((part: { type?: string }) => part.type === 'text'))
        )
    }
  } catch {
    return false
  } finally {
    if (fd !== undefined) fs.closeSync(fd)
  }
  return false
}
