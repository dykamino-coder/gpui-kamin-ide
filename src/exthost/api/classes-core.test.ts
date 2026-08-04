// B5 — Position/Range/Selection geometry (real maths, not stubs).
import { describe, expect, it } from "vitest"
import { Position, Range, Selection } from "./classes-core.js"

describe("Position", () => {
  it("orders with isBefore/isAfter/compareTo", () => {
    const a = new Position(1, 2)
    const b = new Position(1, 5)
    const c = new Position(3, 0)
    expect(a.isBefore(b)).toBe(true)
    expect(b.isAfter(a)).toBe(true)
    expect(a.isBefore(c)).toBe(true)
    expect(a.compareTo(b)).toBe(-1)
    expect(b.compareTo(a)).toBe(1)
    expect(a.compareTo(new Position(1, 2))).toBe(0)
    expect(a.isEqual(new Position(1, 2))).toBe(true)
    expect(a.isBeforeOrEqual(new Position(1, 2))).toBe(true)
  })
  it("translate and with return new positions", () => {
    const p = new Position(2, 3)
    expect(p.translate(1, -1)).toEqual(new Position(3, 2))
    expect(p.translate({ characterDelta: 4 })).toEqual(new Position(2, 7))
    expect(p.with(undefined, 0)).toEqual(new Position(2, 0))
    expect(p.with({ line: 9 })).toEqual(new Position(9, 3))
  })
  it("throws on negative coordinates", () => {
    expect(() => new Position(-1, 0)).toThrow(/non-negative/)
    expect(() => new Position(0, -1)).toThrow(/non-negative/)
  })
  it("isPosition / of accept position-like literals", () => {
    expect(Position.isPosition({ line: 1, character: 2 })).toBe(true)
    expect(Position.isPosition({ line: 1 })).toBe(false)
    expect(Position.isPosition(null)).toBe(false)
    expect(Position.of({ line: 4, character: 5 })).toEqual(new Position(4, 5))
    const p = new Position(1, 1)
    expect(Position.of(p)).toBe(p) // identity for real Positions
  })
})

describe("Range", () => {
  it("normalizes start before end", () => {
    const r = new Range(new Position(5, 0), new Position(1, 0))
    expect(r.start).toEqual(new Position(1, 0))
    expect(r.end).toEqual(new Position(5, 0))
  })
  it("supports the 4-number constructor", () => {
    const r = new Range(1, 2, 3, 4)
    expect(r.start).toEqual(new Position(1, 2))
    expect(r.end).toEqual(new Position(3, 4))
  })
  it("accepts position-like literals (provider returns) without NaN", () => {
    const r = new Range({ line: 0, character: 1 } as unknown as Position, { line: 0, character: 5 } as unknown as Position)
    expect(r.start).toEqual(new Position(0, 1))
    expect(r.end).toEqual(new Position(0, 5))
    expect(Range.isRange({ start: { line: 0, character: 0 }, end: { line: 1, character: 0 } })).toBe(true)
    expect(Range.isRange({ start: { line: 0 } })).toBe(false)
  })
  it("isEmpty / isSingleLine", () => {
    expect(new Range(1, 1, 1, 1).isEmpty).toBe(true)
    expect(new Range(1, 0, 1, 9).isSingleLine).toBe(true)
    expect(new Range(1, 0, 2, 0).isSingleLine).toBe(false)
  })
  it("contains positions and ranges", () => {
    const r = new Range(1, 0, 3, 0)
    expect(r.contains(new Position(2, 5))).toBe(true)
    expect(r.contains(new Position(4, 0))).toBe(false)
    expect(r.contains(new Range(1, 5, 2, 5))).toBe(true)
  })
  it("intersection and union", () => {
    const a = new Range(1, 0, 3, 0)
    const b = new Range(2, 0, 5, 0)
    expect(a.intersection(b)?.isEqual(new Range(2, 0, 3, 0))).toBe(true)
    expect(a.union(b).isEqual(new Range(1, 0, 5, 0))).toBe(true)
    expect(a.intersection(new Range(4, 0, 5, 0))).toBeUndefined()
  })
})

describe("Selection", () => {
  it("tracks anchor/active and isReversed; normalizes start/end", () => {
    const sel = new Selection(new Position(3, 0), new Position(1, 0))
    expect(sel.anchor).toEqual(new Position(3, 0))
    expect(sel.active).toEqual(new Position(1, 0))
    expect(sel.isReversed).toBe(true)
    expect(sel.start).toEqual(new Position(1, 0)) // Range half normalized
    expect(sel.end).toEqual(new Position(3, 0))
  })
  it("forward selection is not reversed", () => {
    expect(new Selection(1, 0, 2, 0).isReversed).toBe(false)
  })
})
