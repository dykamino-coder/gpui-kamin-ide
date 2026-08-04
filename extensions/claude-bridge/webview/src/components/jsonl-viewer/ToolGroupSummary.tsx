import type { JSX } from 'preact'
import { JsonlTimestamp } from './JsonlTimestamp'

// Max tool-name chips before the header wraps to a second line.
const MAX_CHIPS = 5

export interface ToolGroupSummaryProps {
  /** [name, count] ranked by count desc — already pretty-printed. */
  ranked: [string, number][]
  totalCalls: number
  /** Assistant turns the run spans. Shown ONLY when >1: for tools issued inside
   *  a single message it's always 1 and says nothing. */
  turns?: number
  expanded: boolean
  onToggle: () => void
  timestamp?: string
  /** Agent-lane colour (entry-level groups only). */
  accent?: string
}

/** The one header a collapsed/expanded tool run gets, wherever it came from.
 *
 *  WHY it's shared: the same event — Claude firing a burst of tools — rendered
 *  two different ways depending on a distinction the user does not care about.
 *  Tools inside ONE message got "2 tool calls [Edit ×2] ›" (chips, chevron, css
 *  classes); tools across SEVERAL messages got "TOOL BURST · 6 calls · 3 turns
 *  [Expand]" (a button, inline styles, its own chip markup). Two components had
 *  drifted apart, each with its own copy of prettyToolName and its own chip cap
 *  (5 vs 6). The grouping LOGIC legitimately differs (within a message vs
 *  across messages — "turns" only means something in the second); the look
 *  should not. */
export function ToolGroupSummary({
  ranked, totalCalls, turns, expanded, onToggle, timestamp, accent,
}: ToolGroupSummaryProps): JSX.Element {
  const showTurns = turns !== undefined && turns > 1
  return (
    <div
      class="tool-group-header"
      role="button"
      tabIndex={0}
      aria-expanded={expanded}
      onClick={onToggle}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onToggle() } }}
    >
      <i class="fas fa-layer-group tool-group-icon" style={accent ? `color:${accent}` : undefined} aria-hidden="true" />
      <span class="tool-group-title">
        {totalCalls} tool call{totalCalls === 1 ? '' : 's'}
        {showTurns && <span class="tool-group-turns"> · {turns} turns</span>}
      </span>
      {/* Chips stay visible while expanded too — they're the summary of what's
          below, and hiding them made the header jump on every toggle. */}
      <span class="tool-group-chips">
        {ranked.slice(0, MAX_CHIPS).map(([name, n]) => (
          <span key={name} class="tool-group-chip">{name}<span class="tool-group-x"> ×{n}</span></span>
        ))}
        {ranked.length > MAX_CHIPS && <span class="tool-group-more">+{ranked.length - MAX_CHIPS}</span>}
      </span>
      {timestamp && <JsonlTimestamp timestamp={timestamp} />}
      <span class="tool-arrow"><i class="fas fa-chevron-right" /></span>
    </div>
  )
}
