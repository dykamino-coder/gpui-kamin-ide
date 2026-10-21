// The chat's mounted render window (INC-2026-0071 / BR-22).
//
// The viewer mounts only the most recent `renderCap` rows of the projected,
// visibility-filtered list and shows "N earlier messages — scroll up to load"
// for the rest. It used to take the last `renderCap` PROJECTED rows and only
// then ask JsonlEntry to draw each one. Two predicates decided the same thing —
// `entryIsVisible` (and the recent-tip bypass in derived-cache) said a row
// occupies a slot, JsonlEntry said whether it draws — so a tail of rows that
// were counted but drew nothing (e.g. ≥150 housekeeping rows after the last
// assistant message) mounted NOTHING while the pill reported every older row:
// an empty chat that came back once the tail moved on.
//
// The window is now defined by what draws: walk back from the end, render each
// row, and stop once `cap` rows actually drew. A non-drawable row can never
// occupy a slot, so a non-empty projection with at least one drawable row always
// mounts its last drawable rows, whatever any predicate upstream decides.

export interface DrawableWindow<T> {
  /** Index in the projected list of the first row inside the window; every row
   *  before it is behind the "earlier messages" pill. */
  start: number
  /** Rows that drew, in list order, with their index in the projected list. */
  drawn: Array<{ index: number; value: T }>
  /** Rows inside the window that drew nothing (skipped, not mounted). */
  nullRows: number
}

export function selectDrawableWindow<R, T>(
  rows: readonly R[],
  cap: number,
  render: (row: R, index: number) => T | null | undefined,
): DrawableWindow<T> {
  const drawn: Array<{ index: number; value: T }> = []
  let nullRows = 0
  let i = rows.length - 1
  for (; i >= 0 && drawn.length < cap; i--) {
    const value = render(rows[i] as R, i)
    if (value == null) nullRows++
    else drawn.push({ index: i, value })
  }
  drawn.reverse()
  return { start: i + 1, drawn, nullRows }
}
