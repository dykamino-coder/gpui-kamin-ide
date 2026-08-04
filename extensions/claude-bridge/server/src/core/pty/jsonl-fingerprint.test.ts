import { mkdtemp, writeFile, rm, appendFile } from 'fs/promises'
import { tmpdir } from 'os'
import { join } from 'path'
import { describe, it, expect, afterAll } from 'vitest'
import { fingerprintTranscript, canResume, isRecordBoundary, recordUuidMatches } from './jsonl-fingerprint'

const dirs: string[] = []
async function tmpFile(content: string): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'fp-'))
  dirs.push(dir)
  const file = join(dir, 'transcript.jsonl')
  await writeFile(file, content, 'utf8')
  return file
}
afterAll(async () => {
  for (const d of dirs) await rm(d, { recursive: true, force: true })
})

const line = (i: number) => JSON.stringify({ uuid: `u${i}`, type: 'user' }) + '\n'
const many = (n: number, from = 0) => Array.from({ length: n }, (_, i) => line(i + from)).join('')

describe('transcript fingerprint', () => {
  it('is unchanged by an append — the mirror is still a prefix', async () => {
    const file = await tmpFile(many(50))
    const before = await fingerprintTranscript(file)
    await appendFile(file, many(10, 50), 'utf8')
    const after = await fingerprintTranscript(file)

    expect(after?.head).toBe(before?.head) // same file
    expect(after!.size).toBeGreaterThan(before!.size) // just longer
    expect(canResume(after, before!.head, before!.size)).toBe(true)
  })

  it('changes when the file is rewritten from the start (compaction)', async () => {
    const file = await tmpFile(many(50))
    const before = await fingerprintTranscript(file)
    await writeFile(file, many(50, 900), 'utf8') // same length, different content
    const after = await fingerprintTranscript(file)

    expect(after?.head).not.toBe(before?.head)
    expect(canResume(after, before!.head, before!.size)).toBe(false) // must resend
  })

  it('refuses to resume past the end of a truncated file', async () => {
    const file = await tmpFile(many(50))
    const full = await fingerprintTranscript(file)
    await writeFile(file, many(5), 'utf8') // truncated, head may survive
    const short = await fingerprintTranscript(file)

    expect(canResume(short, full!.head, full!.size)).toBe(false)
  })

  it('refuses to resume a client that has no fingerprint yet', async () => {
    const file = await tmpFile(many(10))
    const fp = await fingerprintTranscript(file)
    expect(canResume(fp, undefined, 0)).toBe(false)
    expect(canResume(fp, fp!.head, undefined)).toBe(false)
  })

  it('reports nothing for a missing file instead of throwing', async () => {
    expect(await fingerprintTranscript(join(tmpdir(), 'does-not-exist.jsonl'))).toBeNull()
    expect(canResume(null, 'abc', 0)).toBe(false)
  })

  it('resumes from exactly the end of the file (nothing new yet)', async () => {
    const file = await tmpFile(many(20))
    const fp = await fingerprintTranscript(file)
    expect(canResume(fp, fp!.head, fp!.size)).toBe(true)
  })

  it('rejects an offset that a mid-file repair has shifted', async () => {
    // repairJsonl rewrites the transcript before a resume. Dropping a line from
    // the MIDDLE leaves the first KB — and therefore the fingerprint — intact
    // while moving every offset after it. The client's cursor then points into
    // the middle of a record, and the fingerprint alone happily allows it.
    // The file must be comfortably larger than the fingerprint's 1KB head, or
    // ANY edit changes the hash and the hole cannot appear at all. That is
    // precisely why it only bites real transcripts: 85MB, edits far past the
    // head.
    const COUNT = 500
    const DROP_AT = 200 // well beyond the hashed head
    const file = await tmpFile(many(COUNT))
    const original = await fingerprintTranscript(file)
    // Records differ in length (`u0` vs `u499`), so sum the real bytes rather
    // than multiplying one line's size — that would not land on a boundary.
    let cursor = 0
    for (let i = 0; i < 400; i++) cursor += Buffer.byteLength(line(i), 'utf8')

    expect(await isRecordBoundary(file, cursor)).toBe(true)

    const kept = many(COUNT).split('\n').filter(Boolean)
    kept.splice(DROP_AT, 1) // repair drops a record from the middle
    await writeFile(file, kept.join('\n') + '\n', 'utf8')

    const after = await fingerprintTranscript(file)
    expect(after?.head).toBe(original?.head) // fingerprint still agrees…
    expect(canResume(after, original!.head, cursor)).toBe(true) // …and would allow it

    // The `{` screen is NOT enough on its own: these records are all about the
    // same length, so a shift of exactly one record still lands on some other
    // record's opening brace. (On a real transcript, where records vary wildly,
    // the same shift landed mid-record and the screen did catch it — which is
    // exactly why it must not be the only gate.)
    const lastRecordStart = cursor - Buffer.byteLength(line(399), 'utf8')
    expect(await recordUuidMatches(file, lastRecordStart, 'u399')).toBe(false)
  })

  it('confirms an unshifted offset still holds the record the client names', async () => {
    const file = await tmpFile(many(500))
    let start = 0
    for (let i = 0; i < 399; i++) start += Buffer.byteLength(line(i), 'utf8')
    expect(await recordUuidMatches(file, start, 'u399')).toBe(true)
    expect(await recordUuidMatches(file, start, 'u400')).toBe(false)
  })

  it('treats an unclaimed record as nothing to disprove', async () => {
    const file = await tmpFile(many(10))
    expect(await recordUuidMatches(file, undefined, undefined)).toBe(true)
  })

  it('accepts the exact end of the file — nothing new to send', async () => {
    const file = await tmpFile(many(10))
    const fp = await fingerprintTranscript(file)
    expect(await isRecordBoundary(file, fp!.size)).toBe(true)
  })

  it('refuses an offset past the end', async () => {
    const file = await tmpFile(many(10))
    const fp = await fingerprintTranscript(file)
    expect(await isRecordBoundary(file, fp!.size + 100)).toBe(false)
  })
})
