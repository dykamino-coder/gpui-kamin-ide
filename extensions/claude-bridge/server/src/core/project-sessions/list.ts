// project_sessions_list and project_session_subagents (INC-2026-0072).
// Listing stats every candidate but opens only the files of the returned page,
// and then reads only a bounded head and tail of each.

import fs from 'node:fs'
import fsp from 'node:fs/promises'
import path from 'node:path'
import { boolArg, intArg } from './args'
import { redact } from './redact'
import { summarize } from './summary'
import { isPlainFile, listTranscripts, resolveScope, subagentDir, type CallerContext } from './scope'
import { resolveTarget } from './target'
import type { ToolOutput } from './types'

const TITLE_CHARS = 120

function oneLine(s: string, max: number): string {
  const flat = redact(s.replace(/\s+/g, ' ').trim()).text
  return flat.length > max ? flat.slice(0, max) + '…' : flat
}

export async function listSessions(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const limit = intArg(args.limit, 'limit', 50, 1, 200)
  const offset = intArg(args.offset, 'offset', 0, 0, 1_000_000)
  const includeArchived = boolArg(args.include_archived, 'include_archived', true)
  const scope = resolveScope(ctx)
  const refs = listTranscripts(ctx, scope, includeArchived)
    .map((ref) => {
      try {
        const st = fs.lstatSync(ref.file)
        return { ref, size: st.size, mtimeMs: st.mtimeMs, stat: st }
      } catch {
        return null
      }
    })
    .filter((r): r is NonNullable<typeof r> => r !== null)
    .sort((a, b) => b.mtimeMs - a.mtimeMs)
  const page = refs.slice(offset, offset + limit)
  const conversations = []
  for (const { ref, size, mtimeMs, stat } of page) {
    const s = await summarize(ref.file, stat)
    const live = ctx.live.find((l) => l.ids.includes(ref.conversationId))
    const sub = subagentDir(ref)
    let subagents = 0
    if (sub) {
      try {
        subagents = (await fsp.readdir(sub)).filter((n) => /^agent-[A-Za-z0-9_-]+\.jsonl$/.test(n)).length
      } catch {
        /* none */
      }
    }
    conversations.push({
      session_id: ref.conversationId,
      title: s.title ? oneLine(s.title, TITLE_CHARS) : null,
      started_at: s.startedAt,
      last_activity_at: new Date(mtimeMs).toISOString(),
      last_entry_at: s.lastEntryAt,
      size_bytes: size,
      lines: s.lines,
      subagents,
      source: ref.source,
      live: !!live,
      attached: !!live?.attached,
      self: ctx.selfIds.includes(ref.conversationId),
    })
  }
  const body = JSON.stringify(
    {
      project: ctx.cwd || null,
      total: refs.length,
      offset,
      returned: conversations.length,
      next_offset: offset + conversations.length < refs.length ? offset + conversations.length : null,
      conversations,
    },
    null,
    1,
  )
  return { blocks: [body], entries: conversations.length }
}

export async function listSubagents(ctx: CallerContext, args: Record<string, unknown>): Promise<ToolOutput> {
  const limit = intArg(args.limit, 'limit', 100, 1, 500)
  const offset = intArg(args.offset, 'offset', 0, 0, 1_000_000)
  const target = await resolveTarget(ctx, { session_id: args.session_id })
  const dir = subagentDir(target.ref)
  const found: Array<{ id: string; file: string; size: number; mtimeMs: number; stat: fs.Stats }> = []
  if (dir) {
    for (const name of await fsp.readdir(dir)) {
      const m = /^agent-([A-Za-z0-9_-]{1,128})\.jsonl$/.exec(name)
      const file = path.join(dir, name)
      if (!m || !isPlainFile(file)) continue
      const st = await fsp.lstat(file)
      found.push({ id: m[1]!, file, size: st.size, mtimeMs: st.mtimeMs, stat: st })
    }
  }
  found.sort((a, b) => b.mtimeMs - a.mtimeMs)
  const agents = []
  for (const a of found.slice(offset, offset + limit)) {
    let meta: Record<string, unknown> = {}
    const metaFile = path.join(dir!, `agent-${a.id}.meta.json`)
    if (isPlainFile(metaFile) && fs.lstatSync(metaFile).size <= 64 * 1024) {
      try {
        meta = JSON.parse(await fsp.readFile(metaFile, 'utf8')) as Record<string, unknown>
      } catch {
        /* unreadable meta */
      }
    }
    const s = await summarize(a.file, a.stat)
    agents.push({
      agent_id: a.id,
      agent_type: typeof meta.agentType === 'string' ? meta.agentType : null,
      description: typeof meta.description === 'string' ? oneLine(meta.description, 300) : null,
      first_prompt: s.title ? oneLine(s.title, TITLE_CHARS) : null,
      started_at: s.startedAt,
      last_activity_at: new Date(a.mtimeMs).toISOString(),
      size_bytes: a.size,
      lines: s.lines,
    })
  }
  const body = JSON.stringify(
    {
      session_id: target.ref.conversationId,
      total: found.length,
      offset,
      returned: agents.length,
      next_offset: offset + agents.length < found.length ? offset + agents.length : null,
      agents,
    },
    null,
    1,
  )
  return { blocks: [body], conversationId: target.ref.conversationId, self: target.self, entries: agents.length }
}
