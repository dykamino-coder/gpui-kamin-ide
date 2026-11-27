import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { expect, it } from 'vitest'
import { recentConsoleMessages } from './console-history'

it('restores only visible messages in this thread, in saved order, from a bounded tail', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'codex-console-'))
  const file = path.join(dir, 'session.jsonl')
  const row = (role: string, content: unknown, overrides = {}) =>
    JSON.stringify({
      engine: 'codex',
      type: role,
      uuid: 'id',
      sessionId: 'thread',
      timestamp: '2026-01-01T12:00:00Z',
      message: { role, content },
      ...overrides,
    })
  try {
    await writeFile(
      file,
      [
        row('user', 'oldest'),
        row('user', 'x'.repeat(140_000)),
        row('user', 'foreign', { sessionId: 'another' }),
        row('user', 'side', { isSidechain: true }),
        row('user', 'metadata', { isMeta: true }),
        row('assistant', [
          { type: 'thinking', thinking: 'opaque' },
          { type: 'tool_use', name: 'internal' },
        ]),
        row('user', 'first visible'),
        '{torn line',
        row('assistant', [{ type: 'text', text: 'second visible' }]),
      ].join('\n'),
    )
    const result = await recentConsoleMessages(file, 'thread')
    expect(result).toMatch(/first visible[\s\S]+second visible/)
    expect(result).not.toMatch(/oldest|foreign|metadata|opaque|internal|side|x{20}/)
    expect(await recentConsoleMessages(file, 'unavailable')).toBe('')
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
})
