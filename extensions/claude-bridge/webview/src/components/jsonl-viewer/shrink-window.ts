/** Should the grown render window be dropped back to its initial size?
 *
 *  Kept as a pure function because every condition here is a way to get it
 *  wrong: shrinking mid-scroll yanks the reader, shrinking during a scroll-up
 *  load fights the load that is still settling, and shrinking a window that was
 *  never grown re-renders for nothing.
 *
 *  Deliberately NOT gated on streaming. A stream keeps the view pinned to the
 *  bottom, which is precisely when unmounting rows far above is safe — and a
 *  long generation is exactly when the window would otherwise sit at its grown
 *  size indefinitely. */
export function shouldShrinkWindow(opts: {
  atBottom: boolean
  cap: number
  initialCap: number
  loadingMore: boolean
}): boolean {
  if (!opts.atBottom) return false
  if (opts.loadingMore) return false
  return opts.cap > opts.initialCap
}
