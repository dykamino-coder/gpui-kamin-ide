import { describe, expect, it } from 'vitest'
import type { JsonlEntry } from '../../shared/jsonl-types'
import { buildHandoff, HANDOFF_BUDGET_CHARS, HANDOFF_TOOL_PREVIEW_CHARS, transcriptTurns } from './handoff'

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
    expect(handoff!.text).toContain('## Conversation so far')
    expect(handoff!.text).toContain('with Claude and ChatGPT; until now Claude (sonnet) answered')
    expect(handoff!.text).toContain('User: Remember 4217.\n\nAssistant: Remembered.')
  })

  it('keeps the newest turns verbatim and lists the earlier requests when over budget', () => {
    const entries: JsonlEntry[] = [user('The launch code is 4217.'), assistant('OK.')]
    for (let i = 0; i < 40; i++) entries.push(user(`question ${i} ${'q'.repeat(80)}`), assistant(`answer ${i}`))
    const handoff = buildHandoff(entries, source, { budgetChars: 2_000 })
    expect(handoff!.shortened).toBe(true)
    expect(handoff!.text).toContain('the user asked:\n- The launch code is 4217.')
    expect(handoff!.text).toContain('answer 39')
    // Budget for the history; the fixed header and the request list floor add ~1.5k.
    expect(handoff!.text.length).toBeLessThan(2_000 + 1_500)
  })

  it('keeps the end of a newest turn that alone exceeds the budget', () => {
    const handoff = buildHandoff(
      [user('first fact 4217'), assistant('ok'), user(`${'L'.repeat(200_000)} TAIL`)],
      source,
    )
    expect(handoff!.text).toContain(' characters cut …]')
    expect(handoff!.text).toContain('LLLL TAIL\n</conversation-history>')
    expect(handoff!.text.length).toBeLessThan(HANDOFF_BUDGET_CHARS + 1_000)
  })

  it('needs no model and is fast on a long conversation', () => {
    const entries: JsonlEntry[] = []
    for (let i = 0; i < 5_000; i++) entries.push(user(`step ${i} ${'s'.repeat(300)}`), assistant(`done ${i}`))
    const t0 = performance.now()
    buildHandoff(entries, source)
    expect(performance.now() - t0).toBeLessThan(200)
  })
})
