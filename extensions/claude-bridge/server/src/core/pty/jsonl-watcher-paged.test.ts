import { afterEach, expect, it } from 'vitest'
import { appendFile, mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { JsonlWatcher } from './jsonl-watcher'
import type { JsonlEntry, JsonlStatus } from '../../shared/jsonl-types'
import { fingerprintTranscript } from './jsonl-fingerprint'

const watchers: JsonlWatcher[] = []
const dirs: string[] = []
afterEach(async () => {
  for (const w of watchers.splice(0)) w.stop()
  for (const d of dirs.splice(0)) await rm(d, { recursive: true, force: true })
})
const line = (uuid: string) => `${JSON.stringify({ type: 'assistant', uuid, message: { content: uuid } })}\n`

it('reattaches by verified offset, retains the tail and reads records arriving during replay exactly once', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'paged-reattach-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  const prefix = Array.from({ length: 6000 }, (_, i) => line(`row-${i}`)).join('')
  await writeFile(file, prefix)
  const fp = await fingerprintTranscript(file)
  const statuses: JsonlStatus[] = []
  const entries: JsonlEntry[] = []
  let growing: Promise<void> | undefined
  const w = new JsonlWatcher(
    Date.now(),
    (rows) => {
      entries.push(...rows)
      growing ??= appendFile(file, line('arrived-during-replay'))
      return true
    },
    (s) => {
      statuses.push(s)
    },
  )
  watchers.push(w)
  ;(w as unknown as { filePath: string }).filePath = file
  w.replayAll({ mode: 'paged' })
  await expect.poll(() => statuses.some((s) => s.replayComplete)).toBe(true)
  expect(entries.slice(0, 200).at(-1)?.uuid).toBe('row-5999')
  expect(entries.every((r) => !r.uuid?.startsWith('row-0'))).toBe(true)
  await growing
  await (w as unknown as { checkForNewContent(): Promise<void> }).checkForNewContent()
  expect(entries.filter((r) => r.uuid === 'arrived-during-replay')).toHaveLength(1)
  // Warm reconnect: only the delta after the persisted complete-line cursor.
  entries.length = 0
  statuses.length = 0
  w.replayAll({
    mode: 'paged',
    cursor: {
      head: fp!.head,
      pos: Buffer.byteLength(prefix),
      lastUuid: 'row-5999',
      lastPos: Buffer.byteLength(prefix) - Buffer.byteLength(line('row-5999')),
    },
  })
  await expect.poll(() => statuses.some((s) => s.replayComplete)).toBe(true)
  expect(entries.map((r) => r.uuid)).toEqual(['arrived-during-replay'])
})

it('opens none of 300 subagent transcripts until selected and bounds the selected replay', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'paged-subagents-'))
  dirs.push(dir)
  const file = join(dir, 'conv.jsonl')
  await writeFile(file, line('main'))
  const agentsDir = join(dir, 'conv', 'subagents')
  await mkdir(agentsDir, { recursive: true })
  const transcript = Array.from(
    { length: 450 },
    (_, i) => `${JSON.stringify({ type: 'assistant', uuid: `agent-${i}`, message: { content: 'x'.repeat(3900) } })}\n`,
  ).join('')
  for (let i = 0; i < 300; i++) await writeFile(join(agentsDir, `agent-${i}.jsonl`), transcript)
  const deliveries: Array<{ id?: string; rows: JsonlEntry[] }> = []
  const statuses: JsonlStatus[] = []
  const w = new JsonlWatcher(
    Date.now(),
    () => true,
    (s) => {
      statuses.push(s)
    },
    undefined,
    undefined,
    undefined,
    (_name, rows, id) => {
      deliveries.push({ id, rows })
      return true
    },
    undefined,
    undefined,
    undefined,
    { mode: 'paged' },
  )
  watchers.push(w)
  Object.assign(w, { filePath: file, slugDir: dir })
  await w.scanSubagents()
  expect(deliveries).toHaveLength(0)
  expect((w as unknown as { subagentWatchers: Map<string, unknown> }).subagentWatchers.size).toBe(0)
  w.selectSubagent('agent-12')
  await expect.poll(() => deliveries.length).toBeGreaterThan(0)
  expect(deliveries.every((d) => d.id === 'agent-12')).toBe(true)
  expect(deliveries.flatMap((d) => d.rows).length).toBeLessThanOrEqual(200)
  expect(deliveries.flatMap((d) => d.rows).at(-1)?.uuid).toBe('agent-449')
  w.selectSubagent(null)
  expect((w as unknown as { subagentWatchers: Map<string, unknown> }).subagentWatchers.size).toBe(0)
}, 30_000)
