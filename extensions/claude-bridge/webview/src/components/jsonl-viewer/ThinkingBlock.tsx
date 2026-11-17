import type { JSX } from 'preact'
import { JsonlTextBlock } from './JsonlTextBlock'

/** "thought 4 s" from the CLI's thinkingDurationMs; plain "thought" without it. */
export function thoughtLabel(durationMs?: number): string {
  if (typeof durationMs !== 'number' || !Number.isFinite(durationMs) || durationMs < 0) return 'thought'
  return `thought ${Math.max(1, Math.round(durationMs / 1000))} s`
}

/** Model thinking, drawn as ordinary readable message text: the same markdown
 *  renderer and typography as the answer, full width, never truncated. The
 *  lightbulb in the gutter is the only thing that tells it apart from the
 *  answer — there is no "Thinking" label and nothing to expand.
 *
 *  Thinking the API returned without text (a signature only, INC-2026-0098)
 *  draws one quiet "thought N s" line, so the reader still sees that the
 *  model thought here. While a block streams, its text may simply not have
 *  arrived yet, so it draws nothing. `redacted_thinking` keeps its own
 *  compact fallback in ContentBlocks. */
export function ThinkingBlock(
  { thinking, isStreaming, durationMs }: { thinking: string; isStreaming?: boolean; durationMs?: number },
): JSX.Element | null {
  if (!thinking || !thinking.trim()) {
    if (isStreaming) return null
    const label = thoughtLabel(durationMs)
    return (
      <div class="block-thinking thinking-hidden">
        <i class="fas fa-lightbulb thinking-mark" role="img" aria-label="Thinking" title="Thinking" />
        <span class="thinking-note" title="The model's thinking was not returned as text">{label}</span>
      </div>
    )
  }
  return (
    <div class="block-thinking">
      <i class="fas fa-lightbulb thinking-mark" role="img" aria-label="Thinking" title="Thinking" />
      <JsonlTextBlock text={thinking} isStreaming={isStreaming} />
    </div>
  )
}
