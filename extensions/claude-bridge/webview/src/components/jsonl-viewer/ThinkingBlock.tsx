import type { JSX } from 'preact'
import { JsonlTextBlock } from './JsonlTextBlock'

/** Model thinking, drawn as ordinary readable message text: the same markdown
 *  renderer and typography as the answer, full width, never truncated. The
 *  lightbulb in the gutter is the only thing that tells it apart from the
 *  answer — there is no "Thinking" label and nothing to expand.
 *
 *  Thinking with no text (signature only / stripped) draws nothing, so a
 *  lightbulb never stands alone; `redacted_thinking` keeps its own compact
 *  fallback in ContentBlocks. */
export function ThinkingBlock(
  { thinking, isStreaming }: { thinking: string; isStreaming?: boolean },
): JSX.Element | null {
  if (!thinking || !thinking.trim()) return null
  return (
    <div class="block-thinking">
      <i class="fas fa-lightbulb thinking-mark" role="img" aria-label="Thinking" title="Thinking" />
      <JsonlTextBlock text={thinking} isStreaming={isStreaming} />
    </div>
  )
}
