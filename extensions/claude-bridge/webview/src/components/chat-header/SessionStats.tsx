import type { JSX } from 'preact'
import { useComputed } from '@preact/signals'
import { jsonlEntriesByTab, getJsonlStructureVersion } from '../../signals/jsonl'
import { activeTabId, tabs } from '../../signals/tabs'
import { compactSegments, activeSegmentIdx } from '../../signals/compact-segments'
import { replayProgressByTab } from '../../signals/replay-progress'
import { useBridge } from '../../hooks/useBridge'
import { computeContextStats, formatCost } from '../../utils/session-cost'

// Context pressure bands — reused by color AND by /compact prompt. 70%
// is the "auto-compact zone" CLI hits by default; we expose a one-click
// button the moment we cross it so users don't get surprised by a
// mid-turn auto-compact.
const WARN_PCT = 70
const DANGER_PCT = 90
// Absolute-token triggers for the big-context models. On a 1M-context tier 70%
// is 700k — a lot of conversation to lose in one compaction. Warn (orange) from
// half a million tokens, danger (red) from 700k — independent of the
// percentage bands, which still cover the 200K-window models.
const EARLY_COMPACT_TOKENS = 500_000
const DANGER_COMPACT_TOKENS = 700_000

// Instructions piped after `/compact` when the user clicks Compact. `/compact
// [instructions]` steers the SUMMARY the CLI writes over the conversation — it
// does NOT run tools, so it can't write a file to disk; anything asking it to
// "save to a directory" is silently ignored. So these guide the summary toward a
// clean hand-off instead: what a fresh agent needs to continue, artifacts by
// reference not copy, secrets redacted.
// Кэш последнего пересчёта (см. мемо в useComputed ниже): один активный таб —
// одной ячейки достаточно.
const statsCache: { key: string; value: ReturnType<typeof computeContextStats> | null } = {
  key: '',
  value: null,
}

const HANDOFF_COMPACT_INSTRUCTIONS =
  "Summarise this conversation as a hand-off for a fresh agent to continue the work. " +
  "Capture the current goal, what is done, what is in progress, and the immediate next step. " +
  "Reference specs, plans, commits, diffs and files by path or identifier rather than restating them. " +
  "Include a short \"suggested skills\" note for what the next agent should invoke. " +
  "Redact any secrets, tokens, or personal data."

/** Compact horizontal stats strip: current-turn context usage bar +
 *  cumulative session cost. Wedged into the chat header so it's always
 *  visible without pulling focus. Click the "Compact" button to pipe a
 *  `/compact` slash command straight to the PTY when context is high. */
export function SessionStats(): JSX.Element | null {
  const bridge = useBridge()

  const stats = useComputed(() => {
    const id = activeTabId.value
    if (!id) return null
    const allEntries = jsonlEntriesByTab.value.get(id) ?? []
    // Мемо по структуре: usage приходит только со СТРУКТУРНЫМ аппендом,
    // текстовые rAF-дельты стрима её не меняют — а computed дёргается на
    // каждый флаш и гнал O(N) computeContextStats по сегменту каждый кадр
    // (аудит #70 D2; тот же паттерн, что segCache в compact-segments).
    const replayingNow = replayProgressByTab.value.get(id) != null
    const segIdxNow = replayingNow ? 0 : activeSegmentIdx.value
    const model = tabs.value.find(t => t.id === id)?.model
    const key = `${id}|${getJsonlStructureVersion(id)}|${segIdxNow}|${replayingNow}|${model ?? ''}`
    if (statsCache.key === key) return statsCache.value
    // While the transcript is still replaying, the segment set and every
    // boundary index are in flux (reverse-load prepends older history), so a
    // segment-scoped stat visibly JUMPS between compacts and the number churns.
    // Bypass segmentation during replay: compute over the resident store as the
    // live segment — a value that only converges, never leaps to an old compact.
    // Restrict stats to the currently-selected compact segment so the
    // context-pressure bar AND the cost reflect just the conversation the user is
    // actually looking at. On "Current" (default) this is the post-compact range;
    // on an archived segment the user sees what THAT sub-conversation cost / how
    // full it got before compaction kicked in.
    const segs = replayingNow ? [] : compactSegments.value
    const segIdx = segIdxNow
    const range = segs.length > 0 ? segs[segIdx] : null
    const entries = range ? allEntries.slice(range.from, range.to) : allEntries
    // The live "Current" segment (the last one, or the whole session when there
    // are no compactions) reflects the CURRENT window; an archived one its peak.
    const isLiveSegment = segs.length === 0 || segIdx === segs.length - 1
    const out = computeContextStats(entries, isLiveSegment, model)
    statsCache.key = key
    statsCache.value = out
    return out
  })

  const s = stats.value
  if (!s) return null

  // The cost is a CUMULATIVE sum over the segment. While history is still
  // replaying it is computed over the resident store, which grows as older
  // compacts stream in — so the number creeps up and then snaps down to the
  // current-segment total the instant replay completes and segmentation kicks
  // back in. That visible churn is the "деньги прыгали" report. `used`/`pct`
  // come from the LATEST turn (loaded first, stable), so only the cost is held
  // back: show a placeholder until the sum has settled.
  const replayingId = activeTabId.value
  const costSettling = replayingId ? replayProgressByTab.value.get(replayingId) != null : false

  // Half-a-million tokens is worth offering the hand-off even on a 1M tier
  // still at ~50% — treat it as at least the warn band so the button appears.
  const early = s.used >= EARLY_COMPACT_TOKENS
  const band = (s.pct >= DANGER_PCT || s.used >= DANGER_COMPACT_TOKENS) ? 'danger'
    : (s.pct >= WARN_PCT || early) ? 'warn' : 'ok'
  // Only highlight when we're approaching the limit. Below the warn band
  // the bar is just a muted progress indicator — colour is reserved for
  // signal ("getting close" / "about to auto-compact").
  const barColor = band === 'danger'
    ? 'var(--accent-red)'
    : band === 'warn'
      ? 'var(--accent-orange)'
      : 'var(--text-muted)'
  const pctColor = band === 'ok' ? 'var(--text-secondary)' : barColor

  function sendCompact(): void {
    const id = activeTabId.value
    if (!id) return
    // CLI reads slash commands from stdin; follow with CR so the REPL actually
    // dispatches. `/compact <instructions>` steers the summary toward a clean
    // hand-off (see HANDOFF_COMPACT_INSTRUCTIONS).
    bridge.sendInput(id, `/compact ${HANDOFF_COMPACT_INSTRUCTIONS}\r`)
  }

  return (
    <div
      class="session-stats"
      data-tooltip={`Context: ${formatTokens(s.used)} / ${formatTokens(s.limit)} · Session cost ≈ ${costSettling ? 'calculating…' : formatCost(s.cost)}`}
      style="display:inline-flex;align-items:center;gap:8px;font-size:11px;color:var(--text-muted);line-height:1;font-variant-numeric:tabular-nums"
    >
      <div style="width:60px;height:4px;background:var(--bg-surface);border-radius:var(--radius-xs);overflow:hidden;flex-shrink:0">
        <div
          style={`width:${s.pct}%;height:100%;background:${barColor};transition:width 0.3s;border-radius:var(--radius-xs)`}
        />
      </div>
      <span style={`color:${pctColor};font-weight:600`}>{s.pct}%</span>
      <span style="color:var(--text-disabled)">·</span>
      <span>{formatTokens(s.used)}</span>
      <span style="color:var(--text-disabled)">·</span>
      {costSettling
        ? <span style="color:var(--text-disabled)" title="Session cost finalises once history finishes loading">…</span>
        : <span>{formatCost(s.cost)}</span>}
      {band !== 'ok' && (
        <button
          onClick={sendCompact}
          type="button"
          title="Compact now with a hand-off summary (steers /compact so a fresh agent can continue) — before auto-compact kicks in"
          style={`background:transparent;border:1px solid ${barColor};color:${barColor};padding:2px 8px;border-radius:var(--radius-xs);font-size:10px;cursor:pointer;font-weight:600;letter-spacing:0.02em;text-transform:uppercase`}
        >
          <i class="fas fa-compress" style="margin-right:4px;font-size:10px" />
          Compact
        </button>
      )}
    </div>
  )
}

function formatTokens(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}
