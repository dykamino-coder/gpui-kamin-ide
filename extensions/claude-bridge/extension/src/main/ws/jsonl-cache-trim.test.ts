// The replay cache is capped. It used to drop from the FRONT of the array,
// which was right only while arrival order matched file order.
//
// The replay now delivers the CURRENT dialog first (newest turns first, so the
// chat paints where the user is looking), so the freshest entries sit at the
// front. Front-dropping then evicted exactly the conversation in view: a session
// past the cap — a real one measured at 30 971 entries — would reload into old
// history with today's turns gone.
import { describe, it, expect } from "vitest"
import { trimJsonlCache } from "./handle-server-message"

const CAP = 20_000

/** Entries as the end-first replay delivers them: the newest file positions
 *  arrive first, then progressively older ones. */
function arrivalOrderEndFirst(total: number): { _ord: number }[] {
  const out: { _ord: number }[] = []
  const BATCH = 200
  for (let end = total; end > 0; end -= BATCH) {
    const start = Math.max(0, end - BATCH)
    for (let i = start; i < end; i++) out.push({ _ord: i })
  }
  return out
}

describe("replay cache trim", () => {
  it("keeps today's turns when the newest arrived FIRST", () => {
    const cached = arrivalOrderEndFirst(30_971) // the session that exposed this
    trimJsonlCache(cached)

    const ords = cached.map((e) => e._ord)
    expect(cached.length).toBeLessThanOrEqual(CAP)
    // The newest entry in the file must survive — it is the turn on screen.
    expect(Math.max(...ords)).toBe(30_970)
    // …and what was dropped is the OLDEST by file order, not by arrival.
    expect(Math.min(...ords)).toBeGreaterThan(0)
  })

  it("still drops the oldest when arrival order matches file order", () => {
    const cached = Array.from({ length: 25_000 }, (_, i) => ({ _ord: i }))
    trimJsonlCache(cached)
    const ords = cached.map((e) => e._ord)
    expect(Math.max(...ords)).toBe(24_999)
    expect(Math.min(...ords)).toBeGreaterThan(0)
  })

  it("leaves a cache under the cap alone", () => {
    const cached = Array.from({ length: 500 }, (_, i) => ({ _ord: i }))
    trimJsonlCache(cached)
    expect(cached.length).toBe(500)
  })

  it("trims in blocks rather than on every arriving batch", () => {
    // Just over the cap: not worth a pass yet, or every batch would re-trim.
    const cached = Array.from({ length: CAP + 10 }, (_, i) => ({ _ord: i }))
    trimJsonlCache(cached)
    expect(cached.length).toBe(CAP + 10)
  })

  it("survives entries with no _ord instead of throwing", () => {
    const cached: { _ord?: number }[] = Array.from({ length: CAP + 2000 }, (_, i) => (
      i % 100 === 0 ? {} : { _ord: i }
    ))
    expect(() => { trimJsonlCache(cached) }).not.toThrow()
    expect(cached.length).toBeLessThanOrEqual(CAP)
  })
})
