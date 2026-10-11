import { afterEach, describe, expect, it } from 'vitest'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { SubagentJsonlWatcher } from './subagent-jsonl-watcher'
import { subagentTranscriptPath } from './subagent-transcript-path'
const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) fs.rmSync(root, { recursive: true, force: true })
})
function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'agent-export-'))
  roots.push(root)
  const directory = path.join(root, 'conversation', 'subagents')
  fs.mkdirSync(directory, { recursive: true })
  const file = path.join(directory, 'agent-example.jsonl')
  fs.writeFileSync(file, '{"uuid":"synthetic"}\n')
  return { root, file, main: path.join(root, 'conversation.jsonl') }
}
describe('session-scoped full subagent archive', () => {
  it('tags stable UTF-8 file positions for bounded replay ordering without timestamps', async () => {
    const f = fixture()
    const first = JSON.stringify({ uuid: 'one', text: 'synthetic ü' }) + '\n'
    const second = JSON.stringify({ uuid: 'two' }) + '\n'
    fs.writeFileSync(f.file, first + second)
    const watcher = new SubagentJsonlWatcher('worker', f.file, () => true) as any
    const all = await watcher.readEntries(0)
    expect(all.map((entry: any) => entry._pos)).toEqual([0, Buffer.byteLength(first)])
    const tail = await watcher.readEntries(Buffer.byteLength(first))
    expect(tail).toEqual([all[1]])
  })
  it('resolves only the requested file of the authenticated conversation', async () => {
    const f = fixture()
    expect(await subagentTranscriptPath(f.main, 'example')).toBe(fs.realpathSync(f.file))
    expect(await subagentTranscriptPath(f.main, 'agent-example')).toBe(fs.realpathSync(f.file))
    expect(await subagentTranscriptPath(path.join(f.root, 'other.jsonl'), 'example')).toBeNull()
    expect(await subagentTranscriptPath(f.main, '../example')).toBeNull()
    expect(await subagentTranscriptPath(f.main, 'worker@team')).toBeNull()
    expect(await subagentTranscriptPath(f.main, 'missing')).toBeNull()
  })
  it('keeps earlier compact-generation archives reachable via server-owned aliases', async () => {
    const f = fixture()
    expect(await subagentTranscriptPath(path.join(f.root, 'tip.jsonl'), 'example', ['conversation'])).toBe(
      fs.realpathSync(f.file),
    )
    expect(await subagentTranscriptPath(path.join(f.root, 'tip.jsonl'), 'example', ['../conversation'])).toBeNull()
  })
})
