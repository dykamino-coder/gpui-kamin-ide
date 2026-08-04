// Auto-scroll-to-bottom + tab-switch scroll memory for the JSONL chat viewer.
//
// Behaviour:
//   - On mount + tab switch: restore the saved scroll position (or anchor to
//     bottom for fresh tabs). `stick` is derived from the ACTUAL landed
//     position, never trusted blindly — this fixes the "physically at bottom
//     but pin stuck false" desync where a clamped restore left the pill up and
//     killed autoscroll.
//   - On new entries OR streaming text growth: snap to bottom only while
//     `stick` holds — never yank the user away mid-scroll.
//   - On user scroll-up: release `stick`; re-stick when the user lands back at
//     the bottom.
//
// Growth detection uses a MutationObserver, NOT a ResizeObserver: the inner
// `.container` is `flex:1` inside the scrolling `.chat-panel`, so its border-box
// is clamped to the panel height and never changes when entries are appended —
// a ResizeObserver on it would never fire (verified live). MutationObserver
// catches both new nodes (childList) and word-by-word streaming (characterData).

import { useEffect, useRef } from 'preact/hooks'
import { activeTabId } from '../../signals/tabs'
import { chatAtBottom } from '../../signals/ui'
import { tabScrollMemory } from '../../signals/scroll-memory'

// Distance (px) from the bottom still counted as "at bottom".
const AT_BOTTOM_PX = 80
// Distance (px) from the top that triggers auto-loading older messages — so the
// user just scrolls up instead of clicking a "show earlier" button.
const NEAR_TOP_PX = 400

export interface ScrollPinHandle {
  containerRef: { current: HTMLDivElement | null }
  /** Scroll the chat surface to the bottom (manual override — re-arms stick). */
  scrollToBottom: () => void
}

export function useChatScrollPin(
  currentTabId: string | undefined,
  entries: ReadonlyArray<unknown>,
  onNearTop?: () => void,
): ScrollPinHandle {
  const onNearTopRef = useRef(onNearTop)
  onNearTopRef.current = onNearTop
  const entriesLength = entries.length
  const containerRef = useRef<HTMLDivElement | null>(null)
  // Whether the view should follow the bottom as content grows.
  const stickRef = useRef<boolean>(true)
  // Guards the scroll event our own scrollTop write generates, so it isn't
  // mistaken for a user scroll (which would recompute + persist state).
  const programmaticRef = useRef<boolean>(false)
  const restoringRef = useRef<boolean>(false)
  // Distance-from-bottom captured just before a scroll-up load — the invariant a
  // prepend preserves. Null when no load is pending. See restoreAnchor.
  const anchorRef = useRef<number | null>(null)
  const prevTopRef = useRef<number>(0)
  // Coalesces the many snap requests a streaming burst fires into one rAF pass.
  const pendingSnapRef = useRef<boolean>(false)
  // Keeps the active-tab id available to the once-attached scroll handler.
  const tabRef = useRef<string | null>(currentTabId ?? null)
  tabRef.current = currentTabId ?? null

  // The real scroll surface is the stable `.chat-panel` wrapper (overflow-y:auto).
  // The inner `.container` has no overflow, so scrollTop on it is a no-op.
  function host(): HTMLElement | null {
    const c = containerRef.current
    if (!c) return null
    return (c.closest('.chat-panel') as HTMLElement | null) ?? c
  }

  function distFromBottom(el: HTMLElement): number {
    return el.scrollHeight - el.scrollTop - el.clientHeight
  }

  // Distance from the BOTTOM captured just before a scroll-up load, or null.
  // See restoreAnchor: this is the invariant a prepend preserves.
  function captureAnchor(): void {
    const el = host()
    if (el) anchorRef.current = el.scrollHeight - el.scrollTop
  }

  /** Keep the reader looking at the same content after older messages are
   *  PREPENDED.
   *
   *  Scrolling up raises the render cap by 400 entries, which grow the document
   *  ABOVE the viewport. scrollTop is measured from the TOP, so leaving it alone
   *  silently moves the reader — the reported "I scroll up, it loads, and throws
   *  me back down". Distance from the BOTTOM is what a prepend leaves untouched,
   *  so we restore that instead. Runs in the growth observer, before paint.
   *  `programmaticRef` keeps the write from being read back as a user scroll
   *  (which would re-arm stick and snap to the end). */
  function restoreAnchor(): boolean {
    const anchor = anchorRef.current
    if (anchor === null) return false
    const el = host()
    if (!el) { anchorRef.current = null; return false }
    // The load only settles once the taller window has laid out; wait for the
    // document to actually be bigger than the anchor we recorded.
    if (el.scrollHeight <= anchor) return true // keep waiting, more is coming
    anchorRef.current = null
    programmaticRef.current = true
    el.scrollTop = el.scrollHeight - anchor
    prevTopRef.current = el.scrollTop
    return true
  }

  function snap(): void {
    const el = host()
    if (!el || !stickRef.current) return
    programmaticRef.current = true
    el.scrollTop = el.scrollHeight
    prevTopRef.current = el.scrollTop
  }

  // Two rAFs: the first lands the scroll, the second re-snaps after any late
  // layout (content-visibility entries measure their real height only once
  // near the viewport, so the first snap can stop a few px short).
  function requestSnap(): void {
    if (pendingSnapRef.current) return
    pendingSnapRef.current = true
    requestAnimationFrame(() => {
      pendingSnapRef.current = false
      snap()
      requestAnimationFrame(snap)
    })
  }

  // Tab switch — restore saved offset, then derive `stick` from where we
  // actually landed (not from the stale saved flag).
  useEffect(() => {
    if (!currentTabId) return
    const saved = tabScrollMemory.get(currentTabId)
    restoringRef.current = true
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const el = host()
        if (!el) { restoringRef.current = false; return }
        if (saved && !saved.atBottom) {
          // Clamp — tab may have shrunk (truncate, /clear) since the save.
          el.scrollTop = Math.max(0, Math.min(saved.scrollTop, el.scrollHeight - el.clientHeight))
        } else {
          el.scrollTop = el.scrollHeight
        }
        const atBottom = distFromBottom(el) < AT_BOTTOM_PX
        stickRef.current = atBottom
        chatAtBottom.value = atBottom
        prevTopRef.current = el.scrollTop
        requestAnimationFrame(() => { restoringRef.current = false })
      })
    })
  }, [currentTabId])

  // New entries arrived OR a streaming entry was replaced in place — snap if
  // sticking. Depends on the array REFERENCE (in-place replacements keep length
  // constant but slice the array).
  useEffect(() => {
    if (stickRef.current) requestSnap()
  }, [entries])

  // Scroll handling + growth observation, wired to the stable scroll host.
  useEffect(() => {
    const h = host()
    if (!h) return
    function onScroll(): void {
      if (!h) return
      if (programmaticRef.current) {
        programmaticRef.current = false
        prevTopRef.current = h.scrollTop
        return
      }
      const top = h.scrollTop
      const movedUp = top < prevTopRef.current - 1
      const atBottom = distFromBottom(h) < AT_BOTTOM_PX
      // A scroll-up releases stick ONLY once it leaves the bottom band. Async
      // content growth (a widget's shadow-DOM render, an image load) makes the
      // browser nudge scrollTop up by a few px, which read as "moved up" and
      // dropped stick — so the Scroll-down pill appeared with the bar visually
      // pinned to the bottom. Treating the whole AT_BOTTOM_PX band as "bottom"
      // (a range, not one exact point) keeps stick armed through that jitter.
      if (movedUp && !atBottom) stickRef.current = false
      else if (atBottom) stickRef.current = true
      chatAtBottom.value = stickRef.current
      prevTopRef.current = top
      if (restoringRef.current) return
      // Scrolled near the top → auto-load the next chunk of older messages
      // (replaces the "show earlier" button — plain scrolling is enough).
      if (movedUp && top < NEAR_TOP_PX) {
        captureAnchor() // …before the window grows under us
        onNearTopRef.current?.()
      }
      const id = tabRef.current
      if (id) tabScrollMemory.set(id, { scrollTop: top, atBottom: stickRef.current })
    }
    h.addEventListener('scroll', onScroll, { passive: true })

    // Growth detector (see file header on why RO can't be used here).
    const target = containerRef.current
    let mo: MutationObserver | null = null
    if (target && typeof MutationObserver !== 'undefined') {
      mo = new MutationObserver(() => {
        // A pending scroll-up load outranks the bottom-snap: the user is reading
        // history, not following the tail.
        if (restoreAnchor()) return
        if (stickRef.current && !restoringRef.current) requestSnap()
      })
      mo.observe(target, { childList: true, subtree: true, characterData: true })
    }

    /** Jump to the end.
     *
     *  `onlyIfPinned` is for callers that fire on their own schedule rather than
     *  on a user's click — a finishing replay, above all. Without it, scrolling
     *  up DURING the initial load ended with the reader thrown back to the
     *  bottom the moment replay-complete arrived: the pin had been correctly
     *  released by the scroll, and this re-armed it regardless. The pill stays
     *  unconditional; a click is a deliberate request. */
    ;(window as any).__scrollChatToBottom = (opts?: { onlyIfPinned?: boolean }) => {
      const el = host()
      if (!el) return
      if (opts?.onlyIfPinned && !stickRef.current) return
      el.scrollTop = el.scrollHeight
      stickRef.current = true
      chatAtBottom.value = true
      prevTopRef.current = el.scrollTop
      const id = tabRef.current
      if (id) tabScrollMemory.set(id, { scrollTop: el.scrollTop, atBottom: true })
    }
    return () => { h.removeEventListener('scroll', onScroll); mo?.disconnect() }
  }, [currentTabId, entriesLength > 0])

  // Mirror the active tab onto the ref each render.
  useEffect(() => { tabRef.current = activeTabId.value ?? null })

  function scrollToBottom(): void {
    const el = host()
    if (!el) return
    el.scrollTop = el.scrollHeight
    stickRef.current = true
    chatAtBottom.value = true
  }

  return { containerRef, scrollToBottom }
}
