import { describe, it, expect } from "vitest"
import { selectTabsToEvict, type TabRetention } from "./jsonl-retention"

const policy = (keep: string[] = ["active"], keepRecentCount = 0) => ({
  keep: new Set(keep),
  budgetEntries: 10_000,
  minTabEntries: 1_000,
  keepRecentCount,
})

const tab = (tabId: string, entries: number, lastActiveSeq: number): TabRetention => ({
  tabId,
  entries,
  lastActiveSeq,
})

describe("selectTabsToEvict", () => {
  it("keeps everything while under budget — an idle pair of tabs costs nothing", () => {
    const tabs = [tab("active", 4_000, 9), tab("old", 3_000, 1)]
    expect(selectTabsToEvict(tabs, policy())).toEqual([])
  })

  it("evicts least-recently-active first, and only until the budget fits", () => {
    const tabs = [
      tab("active", 6_000, 9),
      tab("recent", 5_000, 8),
      tab("stale", 5_000, 2),
      tab("stalest", 5_000, 1),
    ] // 21k total, budget 10k
    // stalest (16k) then stale (11k) then recent (6k) — stops as soon as it fits
    expect(selectTabsToEvict(tabs, policy())).toEqual(["stalest", "stale", "recent"])
  })

  it("never evicts the active tab, even when it alone blows the budget", () => {
    const tabs = [tab("active", 50_000, 9), tab("other", 2_000, 1)]
    expect(selectTabsToEvict(tabs, policy())).toEqual(["other"]) // active survives
  })

  it("never evicts a protected tab (mid-response — its live stream would be lost)", () => {
    const tabs = [tab("active", 6_000, 9), tab("working", 9_000, 3), tab("idle", 4_000, 1)]
    const got = selectTabsToEvict(tabs, policy(["active", "working"]))
    expect(got).toEqual(["idle"])
    expect(got).not.toContain("working")
  })

  it("leaves small tabs alone — refilling them costs more than they free", () => {
    const tabs = [tab("active", 9_000, 9), tab("tiny", 900, 2), tab("big", 3_000, 1)]
    expect(selectTabsToEvict(tabs, policy())).toEqual(["big"]) // tiny is under minTabEntries
  })

  it("returns what it can when the budget is unreachable, rather than looping", () => {
    const tabs = [tab("active", 40_000, 9), tab("a", 1_000, 2), tab("b", 1_000, 1)]
    expect(selectTabsToEvict(tabs, policy())).toEqual(["b", "a"]) // still over budget, but done
  })
})

describe("selectTabsToEvict — keepRecentCount (fast ping-pong between big tabs)", () => {
  it("keeps the two big sessions a user flips between resident, even over budget", () => {
    // Two 35k sessions = 70k, well over the 10k test budget. Without recent-
    // protection the just-left one (LRU) is evicted and switching back re-
    // replays for ~10s. With keepRecentCount:3 both survive → instant switch.
    const tabs = [tab("B", 35_000, 9), tab("A", 35_000, 8)]
    expect(selectTabsToEvict(tabs, policy(["B"], 3))).toEqual([])
  })

  it("still drops a genuinely idle third big tab beyond the recent set", () => {
    const tabs = [
      tab("B", 35_000, 9), // active + most recent
      tab("A", 35_000, 8), // the other one being flipped
      tab("C", 30_000, 7), // 3rd most recent — protected by keepRecentCount:3
      tab("idle", 30_000, 1), // beyond the recent set → evictable
    ]
    expect(selectTabsToEvict(tabs, policy(["B"], 3))).toEqual(["idle"])
  })

  it("protects the N most-recent by activity, not by array order", () => {
    const tabs = [
      tab("oldest", 20_000, 1),
      tab("newest", 20_000, 9),
      tab("middle", 20_000, 5),
    ] // 60k, budget 10k, keepRecent 2 → protect newest+middle, evict oldest
    expect(selectTabsToEvict(tabs, policy(["newest"], 2))).toEqual(["oldest"])
  })

  it("keepRecentCount:0 is the old pure-budget behaviour", () => {
    const tabs = [tab("active", 6_000, 9), tab("a", 5_000, 2), tab("b", 5_000, 1)]
    expect(selectTabsToEvict(tabs, policy(["active"], 0))).toEqual(["b", "a"])
  })
})
