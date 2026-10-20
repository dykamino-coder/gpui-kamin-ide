// INC-2026-0071 / BR-22: the Chat panel showed only "N earlier messages — scroll
// up to load" with no rows under it. Synthetic fixtures (no real session text)
// drive the REAL render model (getRenderModel) and the REAL row renderer
// (JsonlEntry) through the viewer's window selection.
//
// Mechanisms reproduced on main before this change:
//  1. the recent-tip set in derived-cache bypassed the whole visibility
//     predicate, so ≥150 housekeeping rows after the last assistant message
//     (here: invisible task_reminder attachments) filled the 150-row window
//     with rows JsonlEntry draws as null → pill, zero rows;
//  2. rows the predicate counts but whose component bails out after mounting
//     (here: empty away_summary) were returned by JsonlEntry as vnodes and
//     mounted as empty wrappers → the same blank window.
import { afterEach, describe, expect, it } from 'vitest'
import type { JSX } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'
import { getRenderModel } from './derived-cache'
import { JsonlEntry } from './JsonlEntry'
import { selectDrawableWindow } from './render-window'
import { getChatWindowDiag, noteChatWindow, resetChatWindowDiag } from './chat-window-diag'

const CAP = 150 // INITIAL_RENDER_CAP in JsonlViewer.tsx
let seq = 0
let version = 100

function conversation(turns: number): { rows: JsonlEntryData[]; parent: string } {
  const rows: JsonlEntryData[] = []
  let parent: string | undefined
  for (let t = 0; t < turns; t++) {
    const u = { type: 'user', uuid: `u${seq++}`, parentUuid: parent, message: { role: 'user', content: [{ type: 'text', text: `question ${t}` }] } } as JsonlEntryData
    const a = { type: 'assistant', uuid: `u${seq++}`, parentUuid: u.uuid, message: { id: `m${seq}`, role: 'assistant', content: [{ type: 'text', text: `answer ${t}` }] } } as JsonlEntryData
    rows.push(u, a)
    parent = a.uuid!
  }
  return { rows, parent: parent! }
}

function tail(parent: string, count: number, make: (uuid: string, parent: string) => JsonlEntryData): JsonlEntryData[] {
  const out: JsonlEntryData[] = []
  for (let k = 0; k < count; k++) {
    const row = make(`u${seq++}`, parent)
    out.push(row)
    parent = row.uuid!
  }
  return out
}

const invisibleAttachment = (uuid: string, parentUuid: string): JsonlEntryData =>
  ({ type: 'attachment', uuid, parentUuid, attachment: { type: 'task_reminder', content: [] } }) as unknown as JsonlEntryData
const emptyAwaySummary = (uuid: string, parentUuid: string): JsonlEntryData =>
  ({ type: 'system', subtype: 'away_summary', content: '', uuid, parentUuid }) as unknown as JsonlEntryData

function model(rows: JsonlEntryData[]) {
  return getRenderModel(`tab-${seq}`, rows, [], 0, ++version)
}

/** What main's viewer mounted: the last CAP projected rows, then JsonlEntry. */
function legacyDrawn(visible: JsonlEntryData[], toolResults: Parameters<typeof JsonlEntry>[0]['toolResults']): number {
  const hidden = Math.max(0, visible.length - CAP)
  return visible.slice(hidden).filter((e) => JsonlEntry({ entry: e, toolResults }) != null).length
}

function textOf(vnode: JSX.Element): string {
  const entry = (vnode.props as { entry?: JsonlEntryData }).entry
  const c = entry?.message?.content
  return Array.isArray(c) ? String((c[0] as { text?: string }).text) : ''
}

afterEach(() => resetChatWindowDiag())

describe('INC-2026-0071: the chat window always mounts the last drawable rows', () => {
  it('housekeeping rows after the last assistant no longer take render slots', () => {
    const { rows, parent } = conversation(300)
    const all = [...rows, ...tail(parent, 200, invisibleAttachment)]
    const m = model(all)

    // The projection ends with the real last answer, not 200 invisible rows.
    expect(m.visibleMerged).toHaveLength(600)
    expect(m.visibleMerged.at(-1)?.type).toBe('assistant')

    const win = selectDrawableWindow(m.visibleMerged, CAP, (e) => JsonlEntry({ entry: e, toolResults: m.toolResults }))
    expect(win.drawn).toHaveLength(CAP)
    expect(win.start).toBe(450) // "450 earlier messages", with the window full of real rows
    expect(textOf(win.drawn.at(-1)!.value)).toBe('answer 299')
  })

  it('rows the predicate counts but that draw nothing are skipped, not mounted', () => {
    const { rows, parent } = conversation(300)
    const all = [...rows, ...tail(parent, 200, emptyAwaySummary)]
    const m = model(all)
    // Still counted by entryIsVisible: this is the predicate/renderer split.
    expect(m.visibleMerged).toHaveLength(800)
    // The old tail slice mounts nothing drawable — the reported blank window.
    expect(legacyDrawn(m.visibleMerged, m.toolResults)).toBe(0)

    const win = selectDrawableWindow(m.visibleMerged, CAP, (e) => JsonlEntry({ entry: e, toolResults: m.toolResults }))
    expect(win.drawn).toHaveLength(CAP)
    expect(win.nullRows).toBe(200)
    expect(textOf(win.drawn.at(-1)!.value)).toBe('answer 299')
    expect(win.start).toBe(450)
  })

  it('a long live tool burst keeps the window populated', () => {
    const { rows, parent } = conversation(50)
    let p = parent
    const burst: JsonlEntryData[] = []
    for (let k = 0; k < 400; k++) {
      const a = { type: 'assistant', uuid: `u${seq++}`, parentUuid: p, message: { id: `t${seq}`, role: 'assistant', content: [{ type: 'tool_use', id: `tool${k}`, name: 'Read', input: {} }] } } as JsonlEntryData
      const r = { type: 'user', uuid: `u${seq++}`, parentUuid: a.uuid, message: { role: 'user', content: [{ type: 'tool_result', tool_use_id: `tool${k}`, content: 'ok' }] } } as JsonlEntryData
      const att = invisibleAttachment(`u${seq++}`, r.uuid!)
      burst.push(a, r, att)
      p = att.uuid!
    }
    const m = model([...rows, ...burst])
    const win = selectDrawableWindow(m.visibleMerged, CAP, (e) => JsonlEntry({ entry: e, toolResults: m.toolResults }))
    expect(win.drawn.length).toBeGreaterThan(0)
    expect(win.drawn.some((d) => d.index === m.visibleMerged.length - 1 || d.index === m.visibleMerged.length - 2)).toBe(true)
  })

  it('JsonlEntry returns null exactly for rows whose component draws nothing', () => {
    const shapes = [
      { type: 'assistant', uuid: 'a', message: { role: 'assistant', content: [{ type: 'text', text: '  ' }, { type: 'thinking', thinking: '' }] } },
      { type: 'system', subtype: 'turn_duration', uuid: 'b' },
      { type: 'system', subtype: 'away_summary', content: '', uuid: 'c' },
      { type: 'system', subtype: 'stop_hook_summary', uuid: 'd' },
      { type: 'system', subtype: 'init', uuid: 'e' },
      { type: 'user', isCompactSummary: true, uuid: 'f', message: { role: 'user', content: '' } },
      { type: 'tool_group', uuid: 'g', _groupEntries: [] },
    ] as unknown as JsonlEntryData[]
    for (const entry of shapes) expect(JsonlEntry({ entry }), String(entry.uuid)).toBeNull()

    const drawable = [
      { type: 'assistant', uuid: 'h', message: { role: 'assistant', content: [{ type: 'text', text: 'hi' }] } },
      { type: 'system', subtype: 'away_summary', content: 'while you were away', uuid: 'i' },
      { type: 'system', subtype: 'model_fallback', uuid: 'j' },
      { type: 'system', subtype: 'init', mcp_server_errors: [{ name: 'x', error: 'y' }], uuid: 'k' },
      { type: 'user', isCompactSummary: true, uuid: 'l', message: { role: 'user', content: 'summary' } },
    ] as unknown as JsonlEntryData[]
    for (const entry of drawable) expect(JsonlEntry({ entry }), String(entry.uuid)).not.toBeNull()
  })
})

describe('selectDrawableWindow', () => {
  it('counts only drawn rows against the cap and reports what it skipped', () => {
    const rows = [1, 0, 2, 0, 0, 3, 4, 0]
    const win = selectDrawableWindow(rows, 3, (v) => (v ? `r${v}` : null))
    expect(win.drawn.map((d) => d.value)).toEqual(['r2', 'r3', 'r4'])
    expect(win.start).toBe(2)
    expect(win.nullRows).toBe(3)
  })

  it('a projection with no drawable row has nothing hidden behind a pill', () => {
    const win = selectDrawableWindow([0, 0, 0], 2, () => null)
    expect(win).toEqual({ start: 0, drawn: [], nullRows: 3 })
  })
})

describe('chat window instrumentation', () => {
  const base = { storeRows: 900, projectedRows: 800, hiddenRows: 0, renderCap: CAP, segments: 0, latestSegment: true }

  it('counts empty-window-with-rows episodes once per episode and keeps numbers only', () => {
    noteChatWindow({ ...base, drawnRows: 150, nullRows: 4 })
    noteChatWindow({ ...base, drawnRows: 0, nullRows: 800 })
    noteChatWindow({ ...base, drawnRows: 0, nullRows: 800 })
    noteChatWindow({ ...base, drawnRows: 12, nullRows: 0 })
    noteChatWindow({ ...base, drawnRows: 0, nullRows: 800 })
    const d = getChatWindowDiag()
    expect(d.emptyWindowWithRows).toBe(2)
    expect(d.emptyWindowRenders).toBe(3)
    expect(d.maxNullRows).toBe(800)
    expect(d.episodes).toHaveLength(2)
    expect(Object.keys(d.episodes[0]!).sort()).toEqual(
      ['at', 'drawnRows', 'hiddenRows', 'latestSegment', 'nullRows', 'projectedRows', 'renderCap', 'segments', 'storeRows'],
    )
  })

  it('an empty store is not an empty-window episode', () => {
    noteChatWindow({ ...base, storeRows: 0, projectedRows: 0, drawnRows: 0, nullRows: 0 })
    expect(getChatWindowDiag().emptyWindowWithRows).toBe(0)
  })

  it('a housekeeping-only store showing the placeholder is not an episode', () => {
    // Fresh or not-found-on-resume session: 8 housekeeping rows, nothing projects.
    const housekeeping = { ...base, storeRows: 8, projectedRows: 0, hiddenRows: 0, drawnRows: 0, nullRows: 0 }
    noteChatWindow(housekeeping)
    noteChatWindow(housekeeping)
    const d = getChatWindowDiag()
    expect(d.emptyWindowWithRows).toBe(0)
    expect(d.emptyWindowRenders).toBe(0)
    expect(d.placeholderRenders).toBe(2)
    expect(d.inEmptyState).toBe(false)
    expect(d.episodes).toHaveLength(0)
  })

  it('rows behind the pill with nothing drawn are an episode', () => {
    noteChatWindow({ ...base, projectedRows: 0, hiddenRows: 650, drawnRows: 0, nullRows: 0 })
    const d = getChatWindowDiag()
    expect(d.emptyWindowWithRows).toBe(1)
    expect(d.placeholderRenders).toBe(0)
  })
})
