// ============================================================================
// One conversation across engines (owner decision 2026-11-12).
//
// A chat tab is ONE logical conversation. Switching the model to the other
// engine (Claude CLI <-> codex app-server) does not open a new chat: the
// conversation becomes a chain of segments, each run by one engine in its own
// native session (Claude CLI conversation / codex thread), e.g.
//   [claude A][codex B][claude C]
// The client sees one transcript: Bridge keeps a single normalized log of the
// whole conversation (unified-log.ts), into which every segment's native
// transcript is copied as it grows (segment-pump.ts). The client's
// conversation id is the logical id; native ids stay on the server, here.
//
// How others do it: chat clients that switch providers mid-conversation
// (Cursor, Zed, Continue, Open WebUI) keep a provider-neutral message list and
// resend it to whichever model answers next. Bridge's engines are agent CLIs
// with their own session stores, so the neutral list is Bridge's own normalized
// transcript (JsonlEntry, one shape for both engines); a new segment starts
// with that history in its session context (handoff.ts).
//
// The manifest lives next to the conversation's own settings directory
// (`conversation.json`), on the same persistent volume as the sessions.
// ============================================================================

import fs from 'node:fs'
import path from 'node:path'
import type { AgentEngineId } from '../../shared/agent-events'

export const MANIFEST_FILE = 'conversation.json'

export interface ConversationSegment {
  engine: AgentEngineId
  /** Native session id (Claude conversation / codex thread); null until known. */
  nativeId: string | null
  /** The segment engine's own settings directory (its transcript slug). */
  settingsDir: string
  model: string | null
  startedAt: string
  /** History the segment's native session started with (stable across its
   *  resumes, so the provider's prompt cache stays valid). */
  handoff: string | null
  /** Copy position in the native transcript (segment-pump.ts). */
  pump: { file: string | null; pos: number }
}

export interface ConversationManifest {
  version: 1
  /** Logical conversation id — the id the client holds. */
  id: string
  /** Settings directory of the conversation itself (holds the unified log). */
  dir: string
  createdAt: string
  segments: ConversationSegment[]
  /** Index of the segment whose engine runs the conversation now. */
  active: number
}

export function manifestPath(dir: string): string {
  return path.join(dir, MANIFEST_FILE)
}

export function readManifest(dir: string): ConversationManifest | null {
  try {
    const m = JSON.parse(fs.readFileSync(manifestPath(dir), 'utf8')) as ConversationManifest
    if (m?.version !== 1 || typeof m.id !== 'string' || !Array.isArray(m.segments) || m.segments.length === 0)
      return null
    if (!(m.active >= 0 && m.active < m.segments.length)) return null
    return m
  } catch {
    return null
  }
}

/** Atomic replace: a torn manifest would orphan the whole conversation. */
export function writeManifest(manifest: ConversationManifest): void {
  fs.mkdirSync(manifest.dir, { recursive: true })
  const file = manifestPath(manifest.dir)
  const tmp = `${file}.${process.pid}.tmp`
  fs.writeFileSync(tmp, JSON.stringify(manifest, null, 1), { mode: 0o600 })
  fs.renameSync(tmp, file)
}

export function activeSegment(manifest: ConversationManifest): ConversationSegment {
  return manifest.segments[manifest.active]!
}

// One in-memory manifest per conversation: the switch and the segment pump
// update the same object, so neither can write back a stale copy.
const loaded = new Map<string, ConversationManifest>()

export function loadManifest(dir: string): ConversationManifest | null {
  const cached = loaded.get(path.resolve(dir))
  if (cached) return cached
  const manifest = readManifest(dir)
  if (manifest) loaded.set(path.resolve(dir), manifest)
  return manifest
}

export function saveManifest(manifest: ConversationManifest): void {
  loaded.set(path.resolve(manifest.dir), manifest)
  writeManifest(manifest)
}

/** Tests: forget cached manifests. */
export function resetManifestCacheForTests(): void {
  loaded.clear()
}
