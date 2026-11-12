// Engine switch inside one chat: the conversation-level steps (manifest.ts).
// Session processes are stopped and started by session-core; everything here
// is file work on the conversation — fast and without any model call, so the
// switch costs no more than starting a chat.
//
// A switch is two-phase. planSwitch only reads: the new segment and its
// history. The joining engine is started next; only when it runs does
// commitSwitch move the conversation to it (seed the log, write the marker,
// update the manifest). A start that fails leaves the conversation as it was,
// with the leaving engine still running.

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import type { AgentEngineId } from '../../shared/agent-events'
import { buildHandoff, readTranscript, type HandoffSource } from '../engine/handoff'
import { computeProjectSlug } from '../pty/jsonl-watcher'
import { archiveTranscript, dropArchivedTranscript, restoreArchivedTranscript } from '../pty/transcript-archive'
import { infoLog, warnLog } from '../logging'
import {
  activeSegment,
  forgetManifest,
  loadManifest,
  saveManifest,
  type ConversationManifest,
  type ConversationSegment,
} from './manifest'
import { SegmentPump } from './segment-pump'
import { UnifiedLog, seedUnifiedLog } from './unified-log'

export interface ConversationRef {
  id: string
  dir: string
}

function projectsDir(): string {
  return path.join(os.homedir(), '.claude', 'projects')
}

/** Slug directory of a settings directory (where its transcripts are). */
export function slugDirOf(settingsDir: string): string {
  return path.join(projectsDir(), computeProjectSlug(settingsDir))
}

/** The conversation's unified log (its client-facing transcript). */
export function unifiedLogPath(ref: ConversationRef): string {
  return path.join(slugDirOf(ref.dir), `${ref.id}.jsonl`)
}

/** Native transcript of a segment (Claude JSONL or Bridge's codex projection). */
function nativeTranscriptPath(segment: ConversationSegment): string | null {
  return segment.nativeId ? path.join(slugDirOf(segment.settingsDir), `${segment.nativeId}.jsonl`) : null
}

const ENGINE_NAME: Record<AgentEngineId, string> = { claude: 'Claude', codex: 'ChatGPT' }

export function engineLabel(engine: AgentEngineId, model: string | null): string {
  return model ? model : ENGINE_NAME[engine]
}

/** The chat the switch leaves: its engine and native session. */
export interface SwitchFrom {
  engine: AgentEngineId
  nativeId: string | null
  settingsDir: string
  model: string | null
}

export interface SwitchPlan {
  ref: ConversationRef
  /** The conversation exists already (a later switch). */
  existing: boolean
  from: SwitchFrom
  segment: ConversationSegment
  label: string
}

/** What a switch to `target` will do — reads only. Null when the chat has no
 *  conversation to carry (no native session yet): the switch is a plain
 *  restart on the other engine. */
export function planSwitch(opts: {
  manifest: ConversationManifest | null
  sessionsDir: string
  from: SwitchFrom
  target: { engine: AgentEngineId; model: string | null }
}): SwitchPlan | null {
  const { manifest, from, target } = opts
  if (!manifest && !from.nativeId) return null
  const id = manifest?.id ?? randomUUID()
  const dir = manifest?.dir ?? path.join(opts.sessionsDir, id)
  const ref = { id, dir }
  const entries = manifest
    ? readTranscript(unifiedLogPath(ref))
    : readTranscript(path.join(slugDirOf(from.settingsDir), `${from.nativeId}.jsonl`))
  const source: HandoffSource = {
    conversationId: id,
    engine: from.engine,
    model: from.model,
    engines: [...new Set([...(manifest?.segments.map((s) => s.engine) ?? []), from.engine])],
  }
  const handoff = buildHandoff(entries, source)
  return {
    ref,
    existing: Boolean(manifest),
    from,
    label: engineLabel(target.engine, target.model),
    segment: {
      engine: target.engine,
      nativeId: null,
      settingsDir: path.join(opts.sessionsDir, randomUUID()),
      model: target.model,
      startedAt: new Date().toISOString(),
      pump: { file: null, pos: 0 },
      pendingContext: handoff?.text ?? null,
      contextBytes: handoff ? Buffer.byteLength(handoff.text) : 0,
    },
  }
}

/** The joining engine runs: move the conversation to its segment. */
export function commitSwitch(plan: SwitchPlan): ConversationManifest {
  let manifest = plan.existing ? loadManifest(plan.ref.dir) : null
  if (!manifest) {
    // First switch: the chat becomes a conversation whose first segment is
    // the chat so far; the unified log starts as a copy of its transcript.
    fs.mkdirSync(plan.ref.dir, { recursive: true })
    const source = path.join(slugDirOf(plan.from.settingsDir), `${plan.from.nativeId}.jsonl`)
    const log = unifiedLogPath(plan.ref)
    seedUnifiedLog(log, source)
    manifest = {
      version: 1,
      id: plan.ref.id,
      dir: plan.ref.dir,
      createdAt: new Date().toISOString(),
      segments: [
        {
          engine: plan.from.engine,
          nativeId: plan.from.nativeId,
          settingsDir: plan.from.settingsDir,
          model: plan.from.model,
          startedAt: new Date().toISOString(),
          pump: { file: source, pos: fs.statSync(log).size },
          logStart: 0,
        },
      ],
      active: 0,
    }
    infoLog('Conversation: chat becomes multi-engine', {
      id: plan.ref.id,
      from: plan.from.nativeId,
      engine: plan.from.engine,
    })
  }
  const log = UnifiedLog.of(unifiedLogPath(plan.ref))
  const segment = { ...plan.segment, startedAt: new Date().toISOString(), logStart: log.size() }
  log.appendSwitchMarker({ engine: segment.engine, model: segment.model, label: plan.label }, plan.ref.id)
  manifest.segments.push(segment)
  manifest.active = manifest.segments.length - 1
  saveManifest(manifest)
  return manifest
}

/** The active segment lost its native session (codex no longer has the
 *  thread; the Claude transcript is gone and has no archived copy): it
 *  continues in a new one, which gets the whole conversation again. */
export function restartSegmentFresh(manifest: ConversationManifest): ConversationSegment {
  const segment = activeSegment(manifest)
  warnLog('Conversation: native session of the segment lost, starting a new one', {
    id: manifest.id,
    engine: segment.engine,
    nativeId: segment.nativeId,
  })
  const previous = manifest.segments[manifest.active - 1] ?? segment
  const handoff = buildHandoff(readTranscript(unifiedLogPath(manifest)), {
    conversationId: manifest.id,
    engine: previous.engine,
    model: previous.model,
    engines: [...new Set(manifest.segments.map((s) => s.engine))],
  })
  segment.nativeId = null
  segment.pump = { file: null, pos: 0 }
  segment.startedAt = new Date().toISOString()
  segment.pendingContext = handoff?.text ?? null
  segment.contextBytes = handoff ? Buffer.byteLength(handoff.text) : 0
  saveManifest(manifest)
  return segment
}

/** Resume of a segment whose Claude transcript the CLI's cleanup swept: put
 *  back the archived copy (INC-2026-0054). False when there is none. */
export async function ensureSegmentTranscript(segment: ConversationSegment): Promise<boolean> {
  const file = nativeTranscriptPath(segment)
  if (!file) return false
  if (fs.existsSync(file)) return true
  return (await restoreArchivedTranscript(projectsDir(), segment.nativeId!)) !== null
}

/** Durable copies of a conversation that ends: its unified log and every
 *  segment's native transcript (INC-2026-0054 for multi-engine chats). */
export async function archiveConversation(ref: ConversationRef): Promise<void> {
  await archiveTranscript(unifiedLogPath(ref), ref.id)
  const manifest = loadManifest(ref.dir)
  for (const segment of manifest?.segments ?? []) {
    const file = nativeTranscriptPath(segment)
    if (file) await archiveTranscript(file, segment.nativeId!)
  }
}

/** Explicit deletion: every segment's directory, native transcript and
 *  archive go with the conversation. `owned` guards what may be removed. */
export function deleteConversationFiles(dir: string, owned: (settingsDir: string) => boolean): void {
  const manifest = loadManifest(dir)
  if (!manifest) return
  for (const segment of manifest.segments) {
    if (segment.nativeId) dropArchivedTranscript(segment.nativeId)
    const file = nativeTranscriptPath(segment)
    if (file) fs.rmSync(file, { force: true })
    if (segment.nativeId)
      fs.rmSync(path.join(slugDirOf(segment.settingsDir), segment.nativeId), { recursive: true, force: true })
    if (segment.settingsDir !== dir && owned(segment.settingsDir))
      fs.rmSync(segment.settingsDir, { recursive: true, force: true })
  }
  fs.rmSync(unifiedLogPath(manifest), { force: true })
  dropArchivedTranscript(manifest.id)
  forgetManifest(dir)
}

/** Hidden context for the active segment's next user turn (the history a new
 *  native session starts with). Taken once; null when there is none. */
export function takeTurnContext(ref: ConversationRef): string | null {
  const manifest = loadManifest(ref.dir)
  if (!manifest) return null
  const segment = activeSegment(manifest)
  if (!segment.pendingContext) return null
  const text = segment.pendingContext
  segment.pendingContext = null
  saveManifest(manifest)
  return text
}

/** Where the running segment's sub-agent transcripts are. */
export function segmentSubagentsDir(ref: ConversationRef): string | null {
  const manifest = loadManifest(ref.dir)
  const segment = manifest ? activeSegment(manifest) : null
  return segment?.nativeId ? path.join(slugDirOf(segment.settingsDir), segment.nativeId, 'subagents') : null
}

/** Copy the running segment's native transcript into the unified log. */
export function startSegmentPump(ref: ConversationRef): SegmentPump | null {
  const manifest = loadManifest(ref.dir)
  if (!manifest) return null
  const segment = activeSegment(manifest)
  let saveTimer: ReturnType<typeof setTimeout> | null = null
  const save = () => {
    if (saveTimer) return
    saveTimer = setTimeout(() => {
      saveTimer = null
      try {
        saveManifest(manifest)
      } catch (err) {
        warnLog('Conversation: manifest save failed', { id: manifest.id, error: String(err) })
      }
    }, 200)
    saveTimer.unref?.()
  }
  const pump = new SegmentPump({
    dir: slugDirOf(segment.settingsDir),
    log: UnifiedLog.of(unifiedLogPath(ref)),
    from: segment.pump,
    // A brand-new segment's transcript is written after it started; the
    // directory is its own, so a small clock margin is enough.
    since: segment.pump.file ? 0 : Date.parse(segment.startedAt) - 5_000,
    onProgress: (at) => {
      segment.pump = at
      save()
    },
    onNativeId: (id) => {
      if (segment.nativeId === id) return
      segment.nativeId = id
      saveManifest(manifest)
    },
    onStop: () => {
      if (saveTimer) clearTimeout(saveTimer)
      saveTimer = null
      saveManifest(manifest)
    },
  })
  pump.start()
  return pump
}

/** The running segment changed its model in place (resume and later switches
 *  use it). */
export function noteSegmentModel(ref: ConversationRef, model: string): void {
  const manifest = loadManifest(ref.dir)
  if (!manifest) return
  activeSegment(manifest).model = model
  saveManifest(manifest)
}
