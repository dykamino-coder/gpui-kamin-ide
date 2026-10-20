// Test fixtures for the project session tools: a real on-disk layout of
// settings dirs, project slug dirs, subagents and the archive under a temp root.

import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { slugOf, type CallerContext } from './scope'

/** Same algorithm as session-settings.ts sanitizeDirName (kept dependency-free
 *  here; the integration test exercises the real one). */
export function sanitize(userPath: string): string {
  return userPath
    .replace(/^[A-Za-z]:/, (m) => m.charAt(0))
    .replace(/[\\/:*?"<>|]+/g, '_')
    .replace(/_+/g, '_')
    .replace(/^_|_$/g, '')
    .slice(0, 80)
}

export interface Layout {
  root: string
  sessionsBase: string
  projectsDir: string
  archiveRoot: string
}

export function makeLayout(): Layout {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'projsess-'))
  const l = {
    root,
    sessionsBase: path.join(root, 'bridge-sessions'),
    projectsDir: path.join(root, 'projects'),
    archiveRoot: path.join(root, 'archive'),
  }
  for (const d of [l.sessionsBase, l.projectsDir, l.archiveRoot]) fs.mkdirSync(d, { recursive: true })
  return l
}

export function settingsDirOf(l: Layout, tokenId: string, cwd: string, sessionId: string): string {
  return path.join(l.sessionsBase, tokenId + (cwd ? '-' + sanitize(cwd) : ''), sessionId)
}

let seq = 0

/** Write a conversation as the server/CLI would; returns its file and slug dir. */
export function addConversation(
  l: Layout,
  tokenId: string,
  cwd: string,
  conversationId: string,
  lines: Array<Record<string, unknown> | string>,
  sessionId = `00000000-0000-4000-8000-${String(++seq).padStart(12, '0')}`,
): { file: string; slugDir: string; settingsDir: string } {
  const settingsDir = settingsDirOf(l, tokenId, cwd, sessionId)
  fs.mkdirSync(settingsDir, { recursive: true })
  const slugDir = path.join(l.projectsDir, slugOf(settingsDir))
  fs.mkdirSync(slugDir, { recursive: true })
  const file = path.join(slugDir, `${conversationId}.jsonl`)
  fs.writeFileSync(file, lines.map((x) => (typeof x === 'string' ? x : JSON.stringify(x))).join('\n') + '\n')
  return { file, slugDir, settingsDir }
}

export function ctxFor(l: Layout, tokenId: string, cwd: string, extra: Partial<CallerContext> = {}): CallerContext {
  return {
    tokenId,
    cwd,
    settingsDir: settingsDirOf(l, tokenId, cwd, 'caller-session'),
    selfIds: [],
    live: [],
    sessionsBase: l.sessionsBase,
    projectsDir: l.projectsDir,
    archiveRoot: l.archiveRoot,
    sanitizeDirName: sanitize,
    ...extra,
  }
}

const T0 = Date.parse('2026-10-20T10:00:00.000Z')
export const at = (minutes: number): string => new Date(T0 + minutes * 60_000).toISOString()

export const user = (text: string, ts: string, extra: Record<string, unknown> = {}) => ({
  type: 'user',
  timestamp: ts,
  message: { role: 'user', content: text },
  ...extra,
})

export const assistant = (
  ts: string,
  content: Array<Record<string, unknown>>,
  extra: { id?: string; model?: string; usage?: Record<string, number> } = {},
) => ({
  type: 'assistant',
  timestamp: ts,
  message: {
    id: extra.id ?? `msg_${++seq}`,
    model: extra.model ?? 'claude-test',
    role: 'assistant',
    content,
    usage: extra.usage ?? { input_tokens: 10, output_tokens: 5 },
  },
})

export const text = (t: string) => ({ type: 'text', text: t })
export const thinking = (t: string) => ({ type: 'thinking', thinking: t, signature: 'sig' })
export const toolUse = (id: string, name: string, input: Record<string, unknown>) => ({
  type: 'tool_use',
  id,
  name,
  input,
})
export const toolResult = (ts: string, id: string, content: string, isError = false) => ({
  type: 'user',
  timestamp: ts,
  message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: id, content, is_error: isError }] },
})

export function trySymlink(target: string, linkPath: string, type: 'dir' | 'file'): boolean {
  try {
    fs.symlinkSync(target, linkPath, type === 'dir' && process.platform === 'win32' ? 'junction' : type)
    return true
  } catch {
    return false
  }
}

/** Every file under `dir` with size and mtime — proves the tools wrote nothing. */
export function snapshotTree(dir: string): string {
  const out: string[] = []
  const walk = (d: string) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name)
      const st = fs.lstatSync(p)
      out.push(`${path.relative(dir, p)} ${st.size} ${st.mtimeMs}`)
      if (e.isDirectory() && !e.isSymbolicLink()) walk(p)
    }
  }
  walk(dir)
  return out.sort().join('\n')
}
