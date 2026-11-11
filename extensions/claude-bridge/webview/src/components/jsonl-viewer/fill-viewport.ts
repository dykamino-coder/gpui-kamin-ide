// When the chat must page older history WITHOUT a scroll gesture.
//
// Older rows are normally pulled by scrolling up (useChatScrollPin → onNearTop).
// That needs a scroll event, and a scroll event needs overflow. A window whose
// drawn rows are shorter than the viewport has none: the server pages history
// by FILE records (200 a page), and 200 records can fold into a handful of
// tool-call groups. Such a window showed a few rows over an empty panel and
// could never be extended — most visibly for a past conversation segment, which
// always arrives as its newest page only (INC-2026-0104).

/** True while the drawn rows do not overflow the scroll host by more than
 *  `nearTopPx`. Past that margin a reader can scroll up into the near-top zone
 *  and still be outside the at-bottom band, so the scroll path takes over. */
export function viewportUnfilled(scrollHeight: number, clientHeight: number, nearTopPx: number): boolean {
  if (clientHeight <= 0) return false // not laid out (hidden panel): no evidence
  return scrollHeight - clientHeight <= nearTopPx
}

/** An older page that lands while the reader is at the bottom is normally stale:
 *  they scrolled back down while it was in flight. A page requested to FILL the
 *  viewport is the opposite case — an unfilled window is always "at the bottom"
 *  — and dropping it is what left the window unfillable. */
export function olderPageIsStale(atBottom: boolean, requestedToFill: boolean): boolean {
  return atBottom && !requestedToFill
}
