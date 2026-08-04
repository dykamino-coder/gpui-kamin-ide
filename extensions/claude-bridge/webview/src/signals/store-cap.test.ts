// The OOM backstop: appendJsonlEntries fires for EVERY tab, but the reader-aware
// trim in JsonlViewer only runs for the ACTIVE, mounted tab. A background session
// mid-response therefore grew without bound — the reported crash was 2 tabs, one
// of them un-trimmed. enforceTabCap closes that: background tabs trim to the
// window on append; the active tab keeps the viewer's larger scroll-up ceiling.
import { describe, it, expect, beforeEach } from 'vitest'
import { appendJsonlEntries, jsonlEntriesByTab, jsonlSeenUuidsByTab, STORE_WINDOW, STORE_SLACK } from './jsonl'
import { activeTabId } from './tabs'
import type { JsonlEntryData } from '../types/jsonl'

const ACTIVE = 'tab-active'
const BG = 'tab-bg'
const len = (tab: string) => (jsonlEntriesByTab.value.get(tab) ?? []).length

// One batch of n user rows, uu/ord-tagged so append neither dedups nor merges.
const batch = (tab: string, n: number): JsonlEntryData[] =>
  Array.from({ length: n }, (_, i) =>
    ({ type: 'user', uuid: `${tab}-u${i}`, _ord: i, message: { content: [] } } as unknown as JsonlEntryData))

beforeEach(() => {
  jsonlEntriesByTab.value = new Map()
  jsonlSeenUuidsByTab.value = new Map()
  activeTabId.value = ACTIVE
})

describe('enforceTabCap (append-time OOM backstop)', () => {
  const over = STORE_WINDOW + STORE_SLACK + 100

  it('trims a BACKGROUND tab to the window on append', () => {
    appendJsonlEntries(BG, batch(BG, over))
    expect(len(BG)).toBe(STORE_WINDOW)
  })

  it('does NOT trim the ACTIVE tab at the same size (viewer owns its trim)', () => {
    appendJsonlEntries(ACTIVE, batch(ACTIVE, over))
    expect(len(ACTIVE)).toBe(over) // well under SCROLL_UP_MAX — left resident
  })

  it('leaves a small background tab untouched', () => {
    appendJsonlEntries(BG, batch(BG, 50))
    expect(len(BG)).toBe(50)
  })
})
