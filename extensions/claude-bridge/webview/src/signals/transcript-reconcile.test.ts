import { beforeEach, describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../types/jsonl'
import { appendJsonlEntries, clearJsonlEntries, getJsonlEntries, getJsonlStructureVersion, prependJsonlEntries, replaceWindowWithSegment } from './jsonl'
import { orderEntries } from './order-entries'
import { getRenderModel } from '../components/jsonl-viewer/derived-cache'
import { selectDrawableWindow } from '../components/jsonl-viewer/render-window'

const TAB = 'transcript-reconcile'
const row = (uuid: string, minute: number, pos?: number): JsonlEntryData => ({
  type: 'user', uuid, timestamp: new Date(Date.UTC(2026, 10, 10, 0, minute)).toISOString(),
  _pos: pos, message: { role: 'user', content: uuid },
} as JsonlEntryData)
const part = (uuid: string, pos: number, parentUuid: string, text: string): JsonlEntryData => ({
  ...row(uuid, 1, pos), type: 'assistant', parentUuid,
  message: { id: 'answer', role: 'assistant', content: [{ type: 'text', text }] },
})
const texts = (entry: JsonlEntryData | undefined) => Array.isArray(entry?.message?.content)
  ? entry.message.content.map(block => block.text) : []
const model = () => getRenderModel(TAB, getJsonlEntries(TAB), [], 0, getJsonlStructureVersion(TAB))

describe('shared transcript reconciliation', () => {
  beforeEach(() => clearJsonlEntries(TAB))

  it('keeps recent prompts and answers in the mounted tail when history lacks file positions', () => {
    const history = Array.from({ length: 151 }, (_, i) => row(`old-${i}`, i))
    const prompt = row('recent-prompt', 180, 9000)
    const answer = { ...part('recent-answer', 9100, prompt.uuid!, 'Recent full answer'),
      timestamp: row('', 181).timestamp }
    appendJsonlEntries(TAB, [prompt, answer])
    appendJsonlEntries(TAB, history.reverse()) // reconnect / mirror delivery
    const visible = model().visibleMerged
    const mounted = selectDrawableWindow(visible, 150, entry => entry)
    expect(mounted.drawn.slice(-2).map(({ value }) => value.uuid)).toEqual(['recent-prompt', 'recent-answer'])
    expect(getJsonlEntries(TAB).map(e => e.uuid)).toEqual([
      ...Array.from({ length: 151 }, (_, i) => `old-${i}`), 'recent-prompt', 'recent-answer',
    ])
  })

  it('merges split canonical blocks by file order across reverse replay batches', () => {
    const prompt = row('prompt', 0, 0)
    const opening = part('opening', 100, 'prompt', 'Opening paragraph')
    const middle = part('middle', 200, 'opening', 'Middle paragraph')
    const ending = part('ending', 300, 'middle', 'Final paragraph')
    const next = { ...row('next-prompt', 2, 400), parentUuid: 'ending' }
    appendJsonlEntries(TAB, [ending, next])
    appendJsonlEntries(TAB, [middle])
    appendJsonlEntries(TAB, [prompt, opening])
    const merged = getJsonlEntries(TAB).find(e => e.message?.id === 'answer')!
    expect(texts(merged)).toEqual(['Opening paragraph', 'Middle paragraph', 'Final paragraph'])
    expect(merged.uuid).toBe('ending')
    expect(merged.parentUuid).toBe('prompt')
    expect(getJsonlEntries(TAB).map(e => e.uuid)).toEqual(['prompt', 'ending', 'next-prompt'])
    appendJsonlEntries(TAB, [opening, middle, ending])
    expect(texts(getJsonlEntries(TAB)[1])).toEqual(texts(merged))
    expect(model().visibleMerged.map(e => e.uuid)).toEqual(['prompt', 'ending', 'next-prompt'])
  })

  it('reconciles a split answer across the older-page boundary before projecting it', () => {
    appendJsonlEntries(TAB, [part('ending', 300, 'middle', 'Final paragraph'), row('newer', 2, 400)])
    const oldModel = model()
    prependJsonlEntries(TAB, [row('prompt', 0, 0), part('opening', 100, 'prompt', 'Opening paragraph'),
      part('middle', 200, 'opening', 'Middle paragraph')])
    const entries = getJsonlEntries(TAB)
    expect(entries).toHaveLength(3)
    expect(texts(entries[1])).toEqual(['Opening paragraph', 'Middle paragraph', 'Final paragraph'])
    const updated = model().visibleMerged.find(e => e.uuid === 'ending')
    expect(updated).not.toBe(oldModel.visibleMerged.find(e => e.uuid === 'ending'))
    expect(texts(updated)).toEqual(texts(entries[1]))
  })

  it('uses the same folding and UUID dedup when replacing an archived segment', () => {
    const prompt = row('prompt', 0, 0)
    const opening = part('opening', 100, 'prompt', 'Opening paragraph')
    const ending = part('ending', 200, 'opening', 'Final paragraph')
    replaceWindowWithSegment(TAB, [ending, prompt, opening])
    expect(getJsonlEntries(TAB).map(e => e.uuid)).toEqual(['prompt', 'ending'])
    expect(texts(getJsonlEntries(TAB)[1])).toEqual(['Opening paragraph', 'Final paragraph'])
    expect(appendJsonlEntries(TAB, [opening, ending])).toBe(false)
  })

  it('keeps file order for fully positioned rows and parent edges through folded UUIDs', () => {
    const parent = { ...row('ending', 10, 100), __uuids: ['opening', 'ending'] }
    const child = { ...row('child', 0, 50), parentUuid: 'opening' }
    const later = row('later', 1, 200)
    expect(orderEntries([later, child, parent]).map(e => e.uuid)).toEqual(['ending', 'child', 'later'])
  })
})
