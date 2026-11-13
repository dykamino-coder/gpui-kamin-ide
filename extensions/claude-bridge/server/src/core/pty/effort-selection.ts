// INC-2026-0099: durable effort choices for a resumed Claude conversation.
// A successful live /effort command changes only that process. Until a newer
// assistant row records its actual effort, the user's choice must win over
// the transcript and the spawn default. Keep the existing model store intact.
import fs from 'fs'
import path from 'path'
import { getDataDir } from '../stats/database/lifecycle'
import { debugLog, warnLog } from '../logging'
import { isEffortLevel, type EffortLevel } from './live-switch'
import { lastEffortEntryForResume } from './session-resume-helpers'

interface Selection {
  effort: EffortLevel
  at: string
}
type Store = Record<string, Selection>
const MAX_ENTRIES = 2000

function effortStorePath(): string {
  return path.join(getDataDir(), 'effort-selection.json')
}

function readEfforts(): Store {
  try {
    const raw = JSON.parse(fs.readFileSync(effortStorePath(), 'utf8')) as unknown
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {}
    return raw as Store
  } catch {
    return {}
  }
}

function writeEfforts(store: Store): void {
  try {
    fs.mkdirSync(path.dirname(effortStorePath()), { recursive: true })
    const tmp = `${effortStorePath()}.tmp`
    fs.writeFileSync(tmp, JSON.stringify(store, null, 2))
    fs.renameSync(tmp, effortStorePath())
  } catch (e) {
    warnLog('Effort selection store write failed', { error: String(e) })
  }
}

/** Remember the effort level the user picked for a conversation. */
export function rememberEffortSelection(conversationId: string, effort: string, at = new Date()): void {
  if (!conversationId || !isEffortLevel(effort)) return
  const store = readEfforts()
  store[conversationId] = { effort, at: at.toISOString() }
  const ids = Object.keys(store)
  if (ids.length > MAX_ENTRIES) {
    ids
      .sort((a, b) => (store[a]!.at < store[b]!.at ? -1 : 1))
      .slice(0, ids.length - MAX_ENTRIES)
      .forEach((id) => delete store[id])
  }
  writeEfforts(store)
  debugLog('Effort selection remembered', { conversationId, effort })
}

export function readEffortSelection(conversationId: string): { effort: string; at: string } | null {
  if (!conversationId) return null
  const entry = readEfforts()[conversationId]
  if (!entry || !isEffortLevel(entry.effort)) return null
  return { effort: entry.effort, at: typeof entry.at === 'string' ? entry.at : '' }
}

export function forgetEffortSelection(conversationId: string): void {
  if (!conversationId) return
  const store = readEfforts()
  if (!(conversationId in store)) return
  delete store[conversationId]
  writeEfforts(store)
}

/** Which effort a resuming Claude session is spawned with: the same rule as
 *  model selection: a newer assistant row wins. Null leaves the caller's default. */
export function effortForResumeWithSelection(
  conversationId: string,
  transcript: { effort: string; timestamp: string } | null,
): string | null {
  const selection = readEffortSelection(conversationId)
  const recorded = transcript && isEffortLevel(transcript.effort) ? transcript : null
  if (!selection) return recorded?.effort ?? null
  if (!recorded) return selection.effort
  if (!recorded.timestamp || !selection.at) return selection.effort
  return recorded.timestamp > selection.at ? recorded.effort : selection.effort
}

/** The `--effort` level for a resumed Claude conversation, or null when
 *  neither a choice nor the transcript names a level the CLI accepts. */
export function effortForResume(settingsDir: string, conversationId: string): string | null {
  const effort = effortForResumeWithSelection(conversationId, lastEffortEntryForResume(settingsDir, conversationId))
  return isEffortLevel(effort) ? effort : null
}
