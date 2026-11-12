import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { afterAll, describe, expect, it } from 'vitest'
import { transcriptHasTurns } from './session-codex'

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'codex-empty-chat-'))
afterAll(() => fs.rmSync(dir, { recursive: true, force: true }))

function transcript(name: string, lines: string[]): string {
  const file = path.join(dir, name)
  fs.writeFileSync(file, lines.join('\n') + '\n')
  return file
}

describe('codex chat without a turn', () => {
  it('is empty with no transcript or only notices', () => {
    expect(transcriptHasTurns(path.join(dir, 'missing.jsonl'))).toBe(false)
    expect(transcriptHasTurns(transcript('notice.jsonl', ['{"type":"system","engine":"codex","content":"x"}']))).toBe(
      false,
    )
  })

  it('had a turn once a user or assistant entry is there', () => {
    expect(transcriptHasTurns(transcript('user.jsonl', ['{"type":"system"}', '{"type":"user"}']))).toBe(true)
    expect(transcriptHasTurns(transcript('assistant.jsonl', ['{"type":"assistant"}']))).toBe(true)
  })

  it('never calls an unreadable transcript empty', () => {
    expect(transcriptHasTurns(transcript('torn.jsonl', ['{"type":"sys']))).toBe(true)
  })
})
