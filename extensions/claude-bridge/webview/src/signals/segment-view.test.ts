// Loading an archived (out-of-window) compact segment: the window is REPLACED
// with the segment's records, and while that snapshot is on screen live entries
// for the tab are dropped (they belong to the current segment, not this view).
import { describe, it, expect, beforeEach } from 'vitest'
import {
  jsonlEntriesByTab, jsonlSeenUuidsByTab, appendJsonlEntries,
  replaceWindowWithSegment, setArchivedView, archivedViewTs,
} from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

const TAB = 't'
const rec = (uuid: string, pos: number, text = 'x'): JsonlEntryData =>
  ({ type: 'user', uuid, _pos: pos, _ord: 999, message: { content: [{ type: 'text', text }] } } as unknown as JsonlEntryData)

const store = () => jsonlEntriesByTab.value.get(TAB) ?? []

beforeEach(() => {
  jsonlEntriesByTab.value = new Map()
  jsonlSeenUuidsByTab.value = new Map()
  setArchivedView(TAB, null)
})

describe('replaceWindowWithSegment', () => {
  it('replaces the resident window with the segment and re-tags _ord ascending', () => {
    appendJsonlEntries(TAB, [rec('live1', 5000), rec('live2', 5100)])
    expect(store()).toHaveLength(2)

    replaceWindowWithSegment(TAB, [rec('old1', 10), rec('old2', 20), rec('old3', 30)])
    expect(store().map((e) => e.uuid)).toEqual(['old1', 'old2', 'old3'])
    expect(store().map((e) => (e as { _ord?: number })._ord)).toEqual([0, 1, 2])
  })

  it('resets the seen-set to exactly the segment (old live uuids forgotten)', () => {
    appendJsonlEntries(TAB, [rec('live1', 5000)])
    replaceWindowWithSegment(TAB, [rec('old1', 10)])
    const seen = jsonlSeenUuidsByTab.value.get(TAB)!
    expect(seen.has('old1')).toBe(true)
    expect(seen.has('live1')).toBe(false)
  })
})

describe('archived-view append gate', () => {
  it('drops live appends while an archived segment is being viewed', () => {
    replaceWindowWithSegment(TAB, [rec('old1', 10), rec('old2', 20)])
    setArchivedView(TAB, '2026-01-01T00:00:00Z')
    expect(archivedViewTs(TAB)).toBe('2026-01-01T00:00:00Z')

    const changed = appendJsonlEntries(TAB, [rec('live-new', 6000)])
    expect(changed).toBe(false)
    expect(store().map((e) => e.uuid)).toEqual(['old1', 'old2']) // view intact
  })

  it('resumes appending once the view returns to Current', () => {
    replaceWindowWithSegment(TAB, [rec('old1', 10)])
    setArchivedView(TAB, 'ts')
    appendJsonlEntries(TAB, [rec('blocked', 6000)])
    expect(store()).toHaveLength(1)

    setArchivedView(TAB, null)
    appendJsonlEntries(TAB, [rec('live-again', 6100)])
    expect(store().map((e) => e.uuid)).toContain('live-again')
  })
})
