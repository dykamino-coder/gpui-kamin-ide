// The host transcript cache mixes tagged rows (page/replay: `_pos`) with
// untagged live-tail rows. Ranking untagged rows as 0 made the cap drop the
// whole live session and the renderer replay cut out the middle of it.
import { describe, it, expect } from "vitest"
import { trimJsonlCache } from "../main/ws/handle-server-message"
import { newestInFileOrder } from "./jsonl-cache-order"
import { chatReplayTail } from "./chat-stored-entries"

type Row = { _pos?: number; _ord?: number; uuid?: string }

/** A paged first page (200 tagged rows), then `live` untagged tail rows. */
function pagedThenLive(live: number): Row[] {
  const rows: Row[] = []
  for (let i = 0; i < 200; i++) rows.push({ _pos: i * 100, _ord: i * 100, uuid: `page-${String(i)}` })
  for (let i = 0; i < live; i++) rows.push({ uuid: `live-${String(i)}` })
  return rows
}

describe("host cache file order with untagged live rows", () => {
  it("trims exactly the overflow, oldest first, instead of every live row", () => {
    const cached = pagedThenLive(21_500) // 21 700 rows: 1 700 over the 20k cap
    trimJsonlCache(cached)
    expect(cached).toHaveLength(20_000)
    // The newest live row (the turn on screen) survives…
    expect(cached.at(-1)?.uuid).toBe("live-21499")
    // …and what went is the oldest: the whole first page, then the oldest live rows.
    expect(cached.some((r) => r.uuid?.startsWith("page-"))).toBe(false)
    expect(cached[0]?.uuid).toBe("live-1500")
  })

  it("replays the newest rows to the renderer as one contiguous tail", () => {
    const cached = pagedThenLive(5_000)
    const tail = newestInFileOrder(cached, 4_500)
    expect(tail).toHaveLength(4_500)
    expect(tail[0]?.uuid).toBe("live-500")
    expect(tail.at(-1)?.uuid).toBe("live-4999")
    // No tagged first-page rows wedged in front of a gap in the live session.
    expect(tail.some((r) => r.uuid?.startsWith("page-"))).toBe(false)
  })

  it("the renderer replay tail (chatReplayTail) keeps the live session contiguous", () => {
    const cached = pagedThenLive(5_000).map((r) => ({ ...r, type: "user" }))
    const tail = chatReplayTail(cached, 4_500)
    expect(tail[0]?.uuid).toBe("live-500")
    expect(tail.at(-1)?.uuid).toBe("live-4999")
    expect(tail.some((r) => r.uuid?.startsWith("page-"))).toBe(false)
  })

  it("keeps file order for fully tagged caches (legacy reverse replay arrival)", () => {
    const cached: Row[] = []
    for (let end = 1000; end > 0; end -= 200) for (let i = end - 200; i < end; i++) cached.push({ _ord: i })
    expect(newestInFileOrder(cached, 3).map((r) => r._ord)).toEqual([997, 998, 999])
  })
})
