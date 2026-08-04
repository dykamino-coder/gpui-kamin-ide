// Phase timeline of OPENING a chat — which step ate the wall-clock, and which
// one never arrived.
//
// A stuck "Loading conversation…" looks identical whether MCP took 40s, the
// replay never reported completion, or the view never bound to the tab. The
// three have different fixes and nothing on screen told them apart, so a report
// could only ever be "it hung". Marks are recorded per tab, FIRST occurrence
// wins (this is the open, not a running log), and the cover renders them — a
// screenshot of the hang is then a full diagnosis, with no devtools involved.
import { signal } from '@preact/signals'

export interface OpenMark {
  /** Stable phase id, e.g. `mcp-ready`. */
  phase: string
  /** ms since this tab's first mark. */
  at: number
}

/** tabId → marks, in the order they happened. */
const timelines = new Map<string, OpenMark[]>()
/** tabId → wall-clock of the first mark, the zero of `at`. */
const origins = new Map<string, number>()

/** Bumped on every new mark so components re-render without subscribing to the
 *  Map itself (a Map mutation is invisible to a signal). */
export const openTimelineTick = signal(0)

/** Record `phase` for `tab` the first time it happens. Later repeats are
 *  ignored: a replay that emits 200 progress events must not bury the shape of
 *  the open in noise. */
export function markOpen(tabId: string | null | undefined, phase: string): void {
  if (!tabId) return
  const marks = timelines.get(tabId) ?? []
  if (marks.some((m) => m.phase === phase)) return
  const now = Date.now()
  if (!origins.has(tabId)) origins.set(tabId, now)
  marks.push({ phase, at: now - (origins.get(tabId) ?? now) })
  timelines.set(tabId, marks)
  openTimelineTick.value++
}

export function getOpenTimeline(tabId: string | null | undefined): OpenMark[] {
  return (tabId && timelines.get(tabId)) || []
}

/** Drop a tab's timeline — on close, or when it is re-opened from scratch. */
export function resetOpenTimeline(tabId: string | null | undefined): void {
  if (!tabId) return
  timelines.delete(tabId)
  origins.delete(tabId)
  openTimelineTick.value++
}

function secs(ms: number): string {
  return ms < 1000 ? `${ms}ms` : `${(ms / 1000).toFixed(1)}s`
}

/** One line, oldest→newest: `activate 0ms · mcp-ready 41.2s · replay-start 41.3s`. */
export function formatOpenTimeline(tabId: string | null | undefined): string {
  return getOpenTimeline(tabId)
    .map((m) => `${m.phase} ${secs(m.at)}`)
    .join(' · ')
}

/** How long we have been stuck: ms since the LAST mark. The single number that
 *  says "this phase is the one that never finished". */
export function stalledForMs(tabId: string | null | undefined): number {
  const marks = getOpenTimeline(tabId)
  const last = marks[marks.length - 1]
  const origin = tabId ? origins.get(tabId) : undefined
  if (!last || origin === undefined) return 0
  return Date.now() - (origin + last.at)
}

/** Name of the phase we are waiting to leave. */
export function lastOpenPhase(tabId: string | null | undefined): string {
  const marks = getOpenTimeline(tabId)
  return marks[marks.length - 1]?.phase ?? '(nothing yet)'
}

// Devtools escape hatch for anyone who does have them open.
try {
  ;(window as unknown as Record<string, unknown>).__openTimeline = (tabId: string) => ({
    marks: getOpenTimeline(tabId),
    line: formatOpenTimeline(tabId),
    stalledMs: stalledForMs(tabId),
  })
} catch {
  /* non-browser (unit tests) */
}
