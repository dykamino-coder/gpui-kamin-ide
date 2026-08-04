// Who is allowed to yank the reader to the end of the chat.
//
// Scrolling up during the initial load ended with the reader thrown back down:
// the scroll correctly released the pin, then replay-complete called the
// jump-to-bottom helper, which re-armed the pin and jumped regardless of where
// the reader was. A replay finishing is not a request from the user; a click on
// the scroll-down pill is.
import { describe, it, expect } from "vitest"

/** Mirrors the helper's guard: `opts?.onlyIfPinned && !pinned` bails out. */
function wouldJump(pinned: boolean, opts?: { onlyIfPinned?: boolean }): boolean {
  if (opts?.onlyIfPinned && !pinned) return false
  return true
}

describe("jump-to-bottom rule", () => {
  it("does NOT jump when replay finishes and the reader has scrolled up", () => {
    expect(wouldJump(false, { onlyIfPinned: true })).toBe(false)
  })

  it("does jump when replay finishes and the reader was still at the end", () => {
    expect(wouldJump(true, { onlyIfPinned: true })).toBe(true)
  })

  it("always jumps for a deliberate click, even after scrolling up", () => {
    // The scroll-down pill passes no options — a click is an explicit request.
    expect(wouldJump(false)).toBe(true)
    expect(wouldJump(true)).toBe(true)
  })
})
