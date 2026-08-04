// A tab whose store we dropped ourselves owes a refill — and owes it even after
// something new lands in it.
//
// Requesting a replay only for an EMPTY store looked sufficient until a real
// session came back with exactly three entries, all of them arriving AFTER the
// clear (a /reload-skills echo), and an EMPTY drop log. Nothing had been
// rejected; the replay had simply never been asked for, because by then the
// store was no longer empty. The session stayed stripped of its history with no
// path back.
import { describe, it, expect, beforeEach } from "vitest"
import { clearJsonlEntries, tabNeedsRefill, clearRefillDebt, appendJsonlEntries, jsonlEntriesByTab } from "./jsonl"
import type { JsonlEntryData } from "../types/jsonl"

const TAB = "tab-1"
const entry = (uuid: string): JsonlEntryData =>
  ({ uuid, type: "system", _ord: 1 } as unknown as JsonlEntryData)

/** Mirrors the viewer's decision on activating a tab. */
function wouldRequestReplay(tabId: string): boolean {
  const existing = jsonlEntriesByTab.value.get(tabId)
  const empty = !existing || existing.length === 0
  return empty || tabNeedsRefill(tabId)
}

describe("refill debt", () => {
  beforeEach(() => {
    jsonlEntriesByTab.value = new Map()
    clearRefillDebt(TAB)
  })

  it("is owed after we drop a tab's store", () => {
    appendJsonlEntries(TAB, [entry("a")])
    clearJsonlEntries(TAB)
    expect(tabNeedsRefill(TAB)).toBe(true)
  })

  it("still requests a replay when a live entry landed after the clear", () => {
    appendJsonlEntries(TAB, [entry("a")])
    clearJsonlEntries(TAB)
    appendJsonlEntries(TAB, [entry("post-clear")]) // the /reload-skills echo
    expect(jsonlEntriesByTab.value.get(TAB)?.length).toBe(1) // no longer empty…
    expect(wouldRequestReplay(TAB)).toBe(true) // …but still asks
  })

  it("stops asking once the replay reported completion", () => {
    clearJsonlEntries(TAB)
    appendJsonlEntries(TAB, [entry("post-clear")])
    clearRefillDebt(TAB)
    expect(wouldRequestReplay(TAB)).toBe(false)
  })

  it("asks for an empty tab even with no debt — the ordinary first load", () => {
    expect(wouldRequestReplay("never-seen")).toBe(true)
  })

  it("keeps the debt per tab, not globally", () => {
    clearJsonlEntries("tab-a")
    expect(tabNeedsRefill("tab-a")).toBe(true)
    expect(tabNeedsRefill("tab-b")).toBe(false)
  })
})
