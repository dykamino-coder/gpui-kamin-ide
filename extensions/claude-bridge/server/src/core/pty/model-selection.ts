// Durable "which model did the user pick for this conversation" (INC-2026-0053).
//
// A hot switch writes `/model <id>` into the live CLI and nothing else. If the
// PTY is respawned before the next assistant response — disconnect/reconnect,
// app restart, effort change — the resume path has no selected model to carry,
// so `createSession` recovers one from the last assistant row in the transcript.
// That row still holds the PREVIOUS model, and the recovered value is passed as
// `--model`, which outranks whatever Claude Code restored. The picker showed the
// new model in the live tab and silently reverted on the next spawn.
//
// The selection is therefore remembered next to the other server state and wins
// over the transcript only while it is NEWER than the last recorded assistant
// response: once the conversation actually answers with some model, that answer
// is the better evidence of what the CLI is using.

import fs from 'fs'
import path from 'path'

import { getDataDir } from '../stats/database/lifecycle'
import { debugLog, warnLog } from '../logging'

interface Selection {
  model: string
  /** ISO time of the switch, compared against the last assistant entry. */
  at: string
}

type Store = Record<string, Selection>

/** Bounded: the file is rewritten whole, and a runaway number of conversations
 *  must not turn it into a multi-megabyte parse on every session start. */
const MAX_ENTRIES = 2000

function storePath(): string {
  return path.join(getDataDir(), 'model-selection.json')
}

function read(): Store {
  try {
    const raw = JSON.parse(fs.readFileSync(storePath(), 'utf8')) as unknown
    if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return {}
    return raw as Store
  } catch {
    return {}
  }
}

function write(store: Store): void {
  try {
    fs.mkdirSync(path.dirname(storePath()), { recursive: true })
    const tmp = `${storePath()}.tmp`
    fs.writeFileSync(tmp, JSON.stringify(store, null, 2))
    fs.renameSync(tmp, storePath())
  } catch (e) {
    warnLog('Model selection store write failed', { error: String(e) })
  }
}

/** Remember an explicit user choice for a conversation. */
export function rememberModelSelection(conversationId: string, model: string, at = new Date()): void {
  if (!conversationId || !model) return
  const store = read()
  store[conversationId] = { model, at: at.toISOString() }
  const ids = Object.keys(store)
  if (ids.length > MAX_ENTRIES) {
    // Oldest choices go first; they are the ones whose conversations have had
    // the most chances to record an assistant response of their own.
    ids
      .sort((a, b) => (store[a]!.at < store[b]!.at ? -1 : 1))
      .slice(0, ids.length - MAX_ENTRIES)
      .forEach((id) => delete store[id])
  }
  write(store)
  debugLog('Model selection remembered', { conversationId, model })
}

export function readModelSelection(conversationId: string): { model: string; at: string } | null {
  if (!conversationId) return null
  const entry = read()[conversationId]
  if (!entry || typeof entry.model !== 'string' || !entry.model) return null
  return { model: entry.model, at: typeof entry.at === 'string' ? entry.at : '' }
}

export function forgetModelSelection(conversationId: string): void {
  if (!conversationId) return
  const store = read()
  if (!(conversationId in store)) return
  delete store[conversationId]
  write(store)
}

/**
 * Which model a resuming session should be spawned with.
 *
 * The user's choice wins only while no assistant response is newer than it.
 * A response carries the model the CLI actually used, so once one exists it is
 * the stronger evidence — otherwise a stale pick would keep overriding a model
 * the conversation has since moved off.
 */
export function modelForResumeWithSelection(
  conversationId: string,
  transcript: { model: string; timestamp: string } | null,
): string | null {
  const selection = readModelSelection(conversationId)
  if (!selection) return transcript?.model ?? null
  if (!transcript) return selection.model
  if (!transcript.timestamp || !selection.at) return selection.model
  return transcript.timestamp > selection.at ? transcript.model : selection.model
}
