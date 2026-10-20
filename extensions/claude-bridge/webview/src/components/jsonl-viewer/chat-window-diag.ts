// Instrumentation for the empty-chat-window state (INC-2026-0071 / BR-22).
//
// "Empty window with rows": the projection of the active tab holds rows that
// should be drawable — rows behind the "N earlier messages" pill or projected
// rows at all — yet the mounted window drew none. On main before this change
// the reported form was that pill with nothing under it. The drawable window
// (render-window.ts) removes the known mechanism, so a non-zero counter now
// means a boundary not yet classified — exactly what BR-22's paired capture
// needs to see.
//
// A store holding only housekeeping rows (a fresh session, a resumed session
// whose transcript was not found) projects to nothing and correctly shows the
// empty "Session Log" placeholder. That is not an episode; it is counted
// separately as `placeholderRenders`.
//
// Counts and sizes only: no ids, text or entry content. Read through the
// stethoscope DiagnosticButton dump, `window.__chatWindowDiag` (shell probe), and
// one console warning per episode.

export interface ChatWindowSample {
  /** Rows retained in the active tab's store. */
  storeRows: number
  /** Rows in the visibility-filtered projection of the active segment. */
  projectedRows: number
  /** Rows the mounted window actually drew. */
  drawnRows: number
  /** Projected rows inside the window that drew nothing (skipped). */
  nullRows: number
  /** Projected rows behind the "earlier messages" pill. */
  hiddenRows: number
  renderCap: number
  /** Number of compact segments and whether the latest one is shown. */
  segments: number
  latestSegment: boolean
}

export interface ChatWindowEpisode extends ChatWindowSample {
  at: string
}

export interface ChatWindowDiag {
  /** Window renders observed with a non-empty store. */
  renders: number
  /** Entries into the empty-window-with-rows state (episodes, not renders). */
  emptyWindowWithRows: number
  /** Renders spent in that state. */
  emptyWindowRenders: number
  /** Renders of a store whose projection is empty (housekeeping rows only):
   *  the legitimate "Session Log" placeholder, not an episode. */
  placeholderRenders: number
  /** Largest number of counted-but-null rows skipped inside one window. */
  maxNullRows: number
  /** Renders whose window skipped at least one counted-but-null row. */
  rendersWithNullRows: number
  inEmptyState: boolean
  /** The last few episodes, newest last. */
  episodes: ChatWindowEpisode[]
}

const MAX_EPISODES = 16

function fresh(): ChatWindowDiag {
  return {
    renders: 0,
    emptyWindowWithRows: 0,
    emptyWindowRenders: 0,
    placeholderRenders: 0,
    maxNullRows: 0,
    rendersWithNullRows: 0,
    inEmptyState: false,
    episodes: [],
  }
}

let diag = fresh()

function publish(): void {
  if (typeof window !== 'undefined') (window as unknown as { __chatWindowDiag?: ChatWindowDiag }).__chatWindowDiag = diag
}

/** Record one window render of the active tab. Called from render; cheap and
 *  transition-based, so repeated renders of one state count one episode. */
export function noteChatWindow(sample: ChatWindowSample, now: () => Date = () => new Date()): void {
  if (sample.storeRows <= 0) {
    diag.inEmptyState = false
    return
  }
  diag.renders += 1
  if (sample.nullRows > 0) {
    diag.rendersWithNullRows += 1
    if (sample.nullRows > diag.maxNullRows) diag.maxNullRows = sample.nullRows
  }
  // Only rows that should be drawable make an empty window an episode: rows
  // behind the pill, or projected rows at all. An empty projection is the
  // housekeeping-only placeholder.
  const shouldDraw = sample.hiddenRows > 0 || sample.projectedRows > 0
  if (sample.drawnRows === 0 && !shouldDraw) diag.placeholderRenders += 1
  const empty = sample.drawnRows === 0 && shouldDraw
  if (empty) {
    diag.emptyWindowRenders += 1
    if (!diag.inEmptyState) {
      diag.emptyWindowWithRows += 1
      diag.episodes.push({ at: now().toISOString(), ...sample })
      if (diag.episodes.length > MAX_EPISODES) diag.episodes.shift()
      // eslint-disable-next-line no-console
      console.warn('[chat-window] empty window with retained rows', sample)
    }
  }
  diag.inEmptyState = empty
  publish()
}

export function getChatWindowDiag(): ChatWindowDiag {
  return { ...diag, episodes: diag.episodes.map((e) => ({ ...e })) }
}

/** Tests only. */
export function resetChatWindowDiag(): void {
  diag = fresh()
  publish()
}
