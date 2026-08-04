import type { JSX } from 'preact'
import { useRef } from 'preact/hooks'
import { renderMarkdown } from '../../utils/render-markdown'

interface JsonlTextBlockProps {
  text: string
  role?: string
  /** Append a blinking cursor at the end (used during MITM streaming). */
  isStreaming?: boolean
}

export function JsonlTextBlock({ text, isStreaming }: JsonlTextBlockProps): JSX.Element {
  // Track text length history to compute the "fresh tail" — the chunk added
  // since the last render. Only the tail receives the blur→focus animation
  // (whole-block flicker would re-blur stable text on every update). Old
  // text passes through markdown unchanged so headers/bold/links stay valid.
  const prevLenRef = useRef(0)
  if (!isStreaming) {
    prevLenRef.current = text.length
    return (
      <div
        class="block-text"
        dangerouslySetInnerHTML={{ __html: renderMarkdown(text) }}
      />
    )
  }
  // Streaming but no text yet — model started a new text-block but the first
  // delta hasn't landed (typical 200-2000ms gap between content_block_start
  // and first content_block_delta). Show a shimmering skeleton so the bubble
  // looks alive instead of a bare blinking cursor on empty space.
  if (text.length === 0) {
    return (
      <div class="block-text block-text-skeleton" aria-hidden="true">
        <span class="skeleton-line" />
        <span class="skeleton-line" />
        <span class="skeleton-line" />
      </div>
    )
  }
  // Streaming render — split into stable head + fresh tail.
  // Heuristic: align split on the last `\n` before the previous-known length.
  // This keeps markdown blocks (paragraphs, code fences) whole on the head,
  // and the tail is plain inline text that we'll animate as the new chunk.
  const prevLen = Math.min(prevLenRef.current, text.length)
  let split = prevLen
  // Walk back to a paragraph boundary so we don't break partial markdown
  // tokens (`**bold` mid-render).
  while (split > 0 && text[split - 1] !== '\n') split--
  const head = text.slice(0, split)
  const tail = text.slice(split)
  prevLenRef.current = text.length
  return (
    <div class="block-text">
      {head && (
        <span
          class="stream-stable"
          dangerouslySetInnerHTML={{ __html: renderMarkdown(head) }}
        />
      )}
      {tail && <StreamingTail text={tail} />}
      <span class="streaming-cursor" aria-hidden="true">▋</span>
    </div>
  )
}

/** Render fresh tail — один span на ДЕЛЬТУ (не на символ) с тем же blur→focus;
 *  показанные чанки держат ключ по стартовой позиции и не переблюриваются. */
const MAX_ANIMATED_TAIL_CHARS = 400

function StreamingTail({ text }: { text: string }): JSX.Element {
  // Inline text only — no markdown for the tail (would need DOM diff). It's
  // a few sentences worth at most until the next `\n` flush merges it back
  // into the stable head. Bold/italic/code ticks render as plain chars
  // momentarily, then get formatted on next flush.
  // Cap the per-char animated window. The head/tail split walks back to the
  // last newline; a long newline-less answer (one big paragraph, common) keeps
  // the WHOLE answer in the tail, so an uncapped per-char wrap would mount O(N)
  // spans every streaming frame and freeze the thread. Only the last
  // MAX_ANIMATED_TAIL_CHARS animate; earlier tail text is one plain span. Keys
  // are absolute positions so a char keeps its node as the window slides.
  // Span на ДЕЛЬТУ, а не на символ (аудит #70 D6): пер-символьная обёртка
  // монтировала до 400 vnode на КАЖДЫЙ стриминг-кадр. Границы дельт ловим
  // диффом с прошлым текстом (rAF-флаш уже коалесцирует поток), каждая
  // дельта — один span с тем же blur→focus; ключ — стартовая позиция, так
  // что показанные чанки не перемонтируются и не переблюриваются. Окно
  // по-прежнему ограничено MAX_ANIMATED_TAIL_CHARS: чанки старше
  // сворачиваются в единый стабильный span.
  const prevRef = useRef('')
  const chunksRef = useRef<Array<{ start: number; end: number }>>([])
  const grew = text.startsWith(prevRef.current)
  if (!grew) chunksRef.current = [] // новый хвост (replace/flush) — сброс
  const from = grew ? prevRef.current.length : 0
  if (text.length > from) chunksRef.current.push({ start: from, end: text.length })
  prevRef.current = text
  const cutoff = Math.max(0, text.length - MAX_ANIMATED_TAIL_CHARS)
  chunksRef.current = chunksRef.current.filter((c) => c.end > cutoff)
  const chunks = chunksRef.current
  const stableEnd = chunks.length ? Math.max(chunks[0].start, cutoff) : text.length
  return (
    <span class="stream-fresh">
      {stableEnd > 0 && <span class="stream-token">{text.slice(0, stableEnd)}</span>}
      {chunks.map((c) => {
        const s = Math.max(c.start, stableEnd)
        if (s >= c.end) return null
        return (
          <span key={c.start} class="stream-token">
            {text.slice(s, c.end)}
          </span>
        )
      })}
    </span>
  )
}
