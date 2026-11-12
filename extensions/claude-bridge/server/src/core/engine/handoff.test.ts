import { describe, expect, it } from 'vitest'
import type { JsonlEntry } from '../../shared/jsonl-types'
import {
  buildHandoff,
  HANDOFF_BUDGET_BYTES,
  HANDOFF_TOOL_PREVIEW_CHARS,
  transcriptTurns,
  withContextBlock,
  withoutContextBlocks,
} from './handoff'

const user = (content: unknown) => ({ type: 'user', message: { role: 'user', content } }) as unknown as JsonlEntry
const assistant = (content: unknown) =>
  ({ type: 'assistant', message: { role: 'assistant', content } }) as unknown as JsonlEntry

const source = { conversationId: '11111111-1111-4111-8111-111111111111', engine: 'claude' as const, model: 'sonnet' }

describe('transcriptTurns', () => {
  it('keeps user and assistant text, tool calls and short results in order', () => {
    const turns = transcriptTurns([
      user('My cat is called Barsik.'),
      assistant([
        { type: 'thinking', thinking: 'secret reasoning' },
        { type: 'text', text: 'Noted.' },
        { type: 'tool_use', id: 't1', name: 'Read', input: { file_path: '/a.txt' } },
      ]),
      user([{ type: 'tool_result', tool_use_id: 't1', content: 'x'.repeat(1000) }]),
      assistant([{ type: 'text', text: 'Done.' }]),
    ])
    expect(turns.map((t) => t.role)).toEqual(['user', 'assistant'])
    expect(turns[0]!.text).toBe('My cat is called Barsik.')
    const reply = turns[1]!.text
    expect(reply).toContain('Noted.')
    expect(reply).toContain('[tool Read] {"file_path":"/a.txt"}')
    expect(reply).toContain(`[Read result] ${'x'.repeat(HANDOFF_TOOL_PREVIEW_CHARS)}… (1000 chars)`)
    expect(reply).toContain('Done.')
    expect(reply).not.toContain('secret reasoning')
  })

  it('leaves out sub-agent side chains, CLI meta lines and system entries', () => {
    const turns = transcriptTurns([
      { ...user('side'), isSidechain: true } as JsonlEntry,
      { ...user('<command-name>/model</command-name>'), isMeta: true } as JsonlEntry,
      { type: 'system', content: 'notice' } as unknown as JsonlEntry,
      user('real question'),
    ])
    expect(turns).toEqual([{ role: 'user', text: 'real question' }])
  })

  it('leaves out the hidden context Bridge added to a user turn', () => {
    const block = buildHandoff([user('old fact'), assistant('ok')], source)!.text
    const sent = withContextBlock('what was the fact?', block)
    expect(withoutContextBlocks(sent)).toBe('what was the fact?')
    expect(transcriptTurns([user(sent), assistant('old fact')])).toEqual([
      { role: 'user', text: 'what was the fact?' },
      { role: 'assistant', text: 'old fact' },
    ])
    // Codex input items and Claude content blocks alike.
    expect(transcriptTurns([user([{ type: 'text', text: sent }])])).toEqual([
      { role: 'user', text: 'what was the fact?' },
    ])
  })
})

describe('buildHandoff', () => {
  it('returns null for a conversation without turns', () => {
    expect(buildHandoff([], source)).toBeNull()
  })

  it('carries a short conversation verbatim and names the engines', () => {
    const handoff = buildHandoff([user('Remember 4217.'), assistant('Remembered.')], {
      ...source,
      engines: ['claude', 'codex'],
    })
    expect(handoff).toMatchObject({ turns: 2, shortened: false })
    expect(handoff!.text.startsWith('<bridge-context>')).toBe(true)
    expect(handoff!.text.endsWith('</bridge-context>')).toBe(true)
    expect(handoff!.text).toContain('not written by the user')
    expect(handoff!.text).toContain('with\nClaude and ChatGPT; until now Claude (sonnet) answered')
    expect(handoff!.text).toContain('User: Remember 4217.\n\nAssistant: Remembered.')
  })

  it('keeps the newest turns verbatim and lists the earlier requests when over budget', () => {
    const entries: JsonlEntry[] = [user('The launch code is 4217.'), assistant('OK.')]
    for (let i = 0; i < 40; i++) entries.push(user(`question ${i} ${'q'.repeat(80)}`), assistant(`answer ${i}`))
    const handoff = buildHandoff(entries, source, { budgetBytes: 3_000 })
    expect(handoff!.shortened).toBe(true)
    expect(handoff!.text).toContain('the user asked:\n- The launch code is 4217.')
    expect(handoff!.text).toContain('answer 39')
    expect(Buffer.byteLength(handoff!.text)).toBeLessThanOrEqual(3_000)
  })

  it('keeps the end of a newest turn that alone exceeds the budget', () => {
    const handoff = buildHandoff(
      [user('first fact 4217'), assistant('ok'), user(`${'L'.repeat(200_000)} TAIL`)],
      source,
    )
    expect(handoff!.text).toContain(' bytes cut …]')
    expect(handoff!.text).toContain('LLLL TAIL\n</conversation-history>')
    expect(Buffer.byteLength(handoff!.text)).toBeLessThanOrEqual(HANDOFF_BUDGET_BYTES)
  })

  it('bounds the block in UTF-8 bytes for Cyrillic text (2 bytes a character)', () => {
    const entries: JsonlEntry[] = []
    for (let i = 0; i < 400; i++)
      entries.push(user(`вопрос ${i} ${'ж'.repeat(300)}`), assistant(`ответ ${i} ${'щ'.repeat(300)}`))
    const handoff = buildHandoff(entries, source)!
    expect(handoff.shortened).toBe(true)
    expect(Buffer.byteLength(handoff.text)).toBeLessThanOrEqual(HANDOFF_BUDGET_BYTES)
    // Far below the 128 KiB a single command-line argument may take, and it is
    // not one anyway: it travels in the first user turn.
    expect(Buffer.byteLength(handoff.text)).toBeLessThan(128 * 1024)
    expect(handoff.text).toContain('ответ 399')
    expect(handoff.text).not.toContain('�')
  })

  it('a huge single message keeps its end within the byte budget', () => {
    const handoff = buildHandoff([user('факт 4217'), assistant('ок'), user(`${'я'.repeat(150_000)} КОНЕЦ`)], source)!
    expect(Buffer.byteLength(handoff.text)).toBeLessThanOrEqual(HANDOFF_BUDGET_BYTES)
    expect(handoff.text).toContain('яяя КОНЕЦ\n</conversation-history>')
    expect(handoff.text).not.toContain('�')
  })

  it('keeps the end of a long paste followed by short turns', () => {
    const log = Array.from({ length: 5_000 }, (_, i) => `log line ${i} heartbeat ok`).join('\n')
    const handoff = buildHandoff(
      [
        user('my fish is Nemo'),
        assistant('noted'),
        user(`Here is a log, reply ok:\n${log}\nFINAL LINE codename MARBLE-FOX-17`),
        assistant('ok'),
        user('what is the codename?'),
      ],
      source,
    )!
    expect(Buffer.byteLength(handoff.text)).toBeLessThanOrEqual(HANDOFF_BUDGET_BYTES)
    expect(handoff.text).toContain('FINAL LINE codename MARBLE-FOX-17\n\nAssistant: ok\n\nUser: what is the codename?')
    expect(handoff.text).toMatch(/User: \[… first \d+ bytes cut …\]/)
    expect(handoff.text).toContain('the user asked:\n- my fish is Nemo')
  })

  it('needs no model and is fast on a long conversation', () => {
    const entries: JsonlEntry[] = []
    for (let i = 0; i < 5_000; i++) entries.push(user(`step ${i} ${'s'.repeat(300)}`), assistant(`done ${i}`))
    const t0 = performance.now()
    buildHandoff(entries, source)
    expect(performance.now() - t0).toBeLessThan(200)
  })
})
