// The mirror is what turns "pull 85MB on every launch" into "read locally, ask
// for the tail". Its whole value depends on never handing back a cursor that
// claims more than it holds — an over-claiming cursor makes the server resume
// PAST records we never stored, and the gap is silent.
import { mkdtemp, rm, readFile, writeFile } from 'fs/promises'
import { tmpdir } from 'os'
import { join } from 'path'
import { describe, it, expect, afterAll } from 'vitest'
import { TranscriptMirror } from './transcript-mirror'

const dirs: string[] = []
async function freshDir(): Promise<string> {
  const d = await mkdtemp(join(tmpdir(), 'mirror-'))
  dirs.push(d)
  return d
}
afterAll(async () => {
  for (const d of dirs) await rm(d, { recursive: true, force: true })
})

const HEAD = 'abc123'
// Records carry BOTH the start and the boundary past them; the cursor must use
// the latter (see the mirror for the off-by-one this prevents).
const rec = (i: number, pos: number, len = 40) => ({ uuid: `u${i}`, type: 'user', _pos: pos, _posEnd: pos + len })

describe('transcript mirror', () => {
  it('reports no cursor before anything is written', async () => {
    const m = new TranscriptMirror(await freshDir(), 'conv')
    expect(await m.readCursor()).toBeNull()
  })

  it('round-trips records and advances the cursor past the last byte offset', async () => {
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(1, 0), rec(2, 100)], HEAD)
    await m.close()

    const cursor = await new TranscriptMirror(dir, 'conv').readCursor()
    expect(cursor?.head).toBe(HEAD)
    expect(cursor?.pos).toBe(140) // exactly the boundary past record 2, not 101
    expect(cursor?.lastUuid).toBe('u2')
  })

  it('appends across sessions instead of rewriting', async () => {
    const dir = await freshDir()
    const first = new TranscriptMirror(dir, 'conv')
    await first.append([rec(1, 0)], HEAD)
    await first.close()

    const second = new TranscriptMirror(dir, 'conv')
    await second.append([rec(2, 50)], HEAD)
    await second.close()

    const body = await readFile(join(dir, 'conv.jsonl'), 'utf8')
    expect(body.trim().split('\n')).toHaveLength(2)
  })

  it('rejects an out-of-order (far-ahead) batch: no write, cursor stays', async () => {
    // Прод-дыра «сообщение не отрисовалось»: tail-preview реплея шлёт ХВОСТ
    // файла первым; append по нему прыгал курсором на конец, и недосланный
    // промежуток навсегда отсекался фильтром `_pos >= cursor`. Батч, не
    // примыкающий к курсору, зеркалу писать нельзя.
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(1, 0), rec(2, 100)], HEAD) // cursor = 140
    await m.append([rec(9, 5_000_000)], HEAD) // far-ahead preview batch → rejected
    await m.close()

    const reopened = new TranscriptMirror(dir, 'conv')
    expect((await reopened.readCursor())?.pos).toBe(140)
    const body = await readFile(join(dir, 'conv.jsonl'), 'utf8')
    expect(body.trim().split('\n')).toHaveLength(2)
    // Примыкающий хвост после отклонённого превью пишется как обычно.
    await reopened.append([rec(3, 140)], HEAD)
    await reopened.close()
    expect((await new TranscriptMirror(dir, 'conv').readCursor())?.pos).toBe(180)
  })

  it('cursor follows the last WRITTEN record, not the batch order', async () => {
    // Батч может прийти неотсортированным: курсор должен взять максимум
    // записанных `_posEnd`, а не последний элемент массива.
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(2, 100), rec(1, 0)], HEAD)
    await m.close()
    expect((await new TranscriptMirror(dir, 'conv').readCursor())?.pos).toBe(140)
  })

  it('reads the tail newest-last, capped', async () => {
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append(Array.from({ length: 50 }, (_, i) => rec(i, i * 10)), HEAD)
    await m.close()

    const tail = await new TranscriptMirror(dir, 'conv').readTail(10)
    expect(tail).toHaveLength(10)
    expect((tail[tail.length - 1] as { uuid: string }).uuid).toBe('u49') // newest last
    expect((tail[0] as { uuid: string }).uuid).toBe('u40')
  })

  it('survives a torn last line rather than throwing', async () => {
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(1, 0)], HEAD)
    await m.close()
    await writeFile(join(dir, 'conv.jsonl'), '{"uuid":"u1"}\n{"uuid":"u2","tr', 'utf8')

    const tail = await new TranscriptMirror(dir, 'conv').readTail(10)
    expect(tail).toHaveLength(1)
  })

  it('drops everything on reset — a rewritten file must not be extended', async () => {
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(1, 0), rec(2, 100)], HEAD)
    await m.reset()

    expect(await m.readCursor()).toBeNull()
    expect(await new TranscriptMirror(dir, 'conv').readTail(10)).toEqual([])
  })

  it('can be written again after a reset', async () => {
    const dir = await freshDir()
    const m = new TranscriptMirror(dir, 'conv')
    await m.append([rec(1, 0)], HEAD)
    await m.reset()
    await m.append([rec(9, 0)], 'newhead')
    await m.close()

    const cursor = await new TranscriptMirror(dir, 'conv').readCursor()
    expect(cursor?.head).toBe('newhead')
    expect(cursor?.lastUuid).toBe('u9')
  })
})
