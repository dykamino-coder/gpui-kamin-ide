// Engine switch inside one chat: the conversation-level steps (manifest.ts).
// Session processes are stopped and started by session-core; everything here
// is file work on the conversation — fast and without any model call, so the
// switch costs no more than starting a chat.

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { randomUUID } from 'node:crypto'
import type { AgentEngineId } from '../../shared/agent-events'
import { buildHandoff, readTranscript } from '../engine/handoff'
import { computeProjectSlug } from '../pty/jsonl-watcher'
import { infoLog, warnLog } from '../logging'
import {
  activeSegment,
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

const ENGINE_NAME: Record<AgentEngineId, string> = { claude: 'Claude', codex: 'ChatGPT' }

export function engineLabel(engine: AgentEngineId, model: string | null): string {
  return model ? model : ENGINE_NAME[engine]
}

/** First switch of a chat: it becomes a conversation whose first segment is
 *  the chat so far. The unified log starts as a copy of its transcript. */
export function beginConversation(opts: {
  sessionsDir: string
  engine: AgentEngineId
  nativeId: string
  settingsDir: string
  model: string | null
}): ConversationManifest {
  const id = randomUUID()
  const dir = path.join(opts.sessionsDir, id)
  fs.mkdirSync(dir, { recursive: true })
  const source = path.join(slugDirOf(opts.settingsDir), `${opts.nativeId}.jsonl`)
  const log = unifiedLogPath({ id, dir })
  seedUnifiedLog(log, source)
  const copied = fs.statSync(log).size
  const manifest: ConversationManifest = {
    version: 1,
    id,
    dir,
    createdAt: new Date().toISOString(),
    segments: [
      {
        engine: opts.engine,
        nativeId: opts.nativeId,
        settingsDir: opts.settingsDir,
        model: opts.model,
        startedAt: new Date().toISOString(),
        handoff: null,
        pump: { file: source, pos: copied },
      },
    ],
    active: 0,
  }
  saveManifest(manifest)
  infoLog('Conversation: chat becomes multi-engine', { id, from: opts.nativeId, engine: opts.engine })
  return manifest
}

/** Next segment on `engine`: a new native session that starts with the whole
 *  conversation so far (budget-cut, see handoff.ts). */
export function addSegment(
  manifest: ConversationManifest,
  target: { engine: AgentEngineId; model: string | null; settingsDir: string },
): ConversationSegment {
  const ref = { id: manifest.id, dir: manifest.dir }
  const log = new UnifiedLog(unifiedLogPath(ref))
  log.appendSwitchMarker({ ...target, label: engineLabel(target.engine, target.model) }, manifest.id)
  const engines = [...new Set(manifest.segments.map((s) => s.engine))]
  const previous = activeSegment(manifest)
  const handoff = buildHandoff(readTranscript(unifiedLogPath(ref)), {
    conversationId: manifest.id,
    engine: previous.engine,
    model: previous.model,
    engines,
  })
  const segment: ConversationSegment = {
    engine: target.engine,
    nativeId: null,
    settingsDir: target.settingsDir,
    model: target.model,
    startedAt: new Date().toISOString(),
    handoff: handoff?.text ?? null,
    pump: { file: null, pos: 0 },
  }
  manifest.segments.push(segment)
  manifest.active = manifest.segments.length - 1
  saveManifest(manifest)
  return segment
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
    log: new UnifiedLog(unifiedLogPath(ref)),
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
