import type { JSX } from 'preact'
import { useMemo } from 'preact/hooks'
import type { ContentBlock } from '../../types/jsonl'
import { hasCliXmlTags } from './utils'
import { CliXmlContent } from './CliXmlContent'
import { JsonlToolResult } from './JsonlToolResult'

/** Match something that looks like a base64 MCP image payload: either a bare
 *  `data:image/...;base64,...` URI or the JSON shape
 *  `{"type":"image","source":{"type":"base64","media_type":"image/png","data":"..."}}`.
 *  Screenshot tools, Read on PNGs/JPGs and vision outputs all land here. */
function extractDataUri(raw: string): string | null {
  const s = raw.trim()
  if (s.startsWith('data:image/')) {
    const m = s.match(/^data:image\/[\w+.-]+;base64,[A-Za-z0-9+/=\s]+$/)
    return m ? s.replace(/\s+/g, '') : null
  }
  // JSON-encoded image content block, possibly wrapped in an array.
  if (s.startsWith('{') || s.startsWith('[')) {
    try {
      const parsed = JSON.parse(s)
      const arr = Array.isArray(parsed) ? parsed : [parsed]
      for (const item of arr) {
        if (item && item.type === 'image' && item.source && item.source.type === 'base64') {
          const media = item.source.media_type || 'image/png'
          return `data:${media};base64,${item.source.data}`
        }
      }
    } catch { /* not JSON */ }
  }
  return null
}

/** Walk a structured content array for any image blocks. Text blocks get
 *  flattened to a single string, images to data: URIs. `tool_reference`
 *  blocks (ToolSearch results — «схема тула подгружена») collect separately
 *  and render as chips instead of raw JSON. */
function splitStructuredContent(blocks: ContentBlock[]): { text: string; images: string[]; toolRefs: string[] } {
  const textParts: string[] = []
  const images: string[] = []
  const toolRefs: string[] = []
  for (const b of blocks) {
    const anyB = b as any
    if (anyB.type === 'image' && anyB.source && anyB.source.type === 'base64') {
      const media = anyB.source.media_type || 'image/png'
      images.push(`data:${media};base64,${anyB.source.data}`)
      continue
    }
    if (anyB.type === 'tool_reference' && typeof anyB.tool_name === 'string') {
      toolRefs.push(anyB.tool_name)
      continue
    }
    if (anyB.text) {
      const embedded = extractDataUri(String(anyB.text))
      if (embedded) { images.push(embedded); continue }
      textParts.push(String(anyB.text))
      continue
    }
    textParts.push(JSON.stringify(anyB))
  }
  return { text: textParts.join('\n'), images, toolRefs }
}

export function ToolResultBlock({ block }: { block: ContentBlock }): JSX.Element {
  // Derive text/images once per content value — extractDataUri JSON.parse's the
  // full string (a multi-MB JSON tool result), and this component re-runs on
  // every JsonlViewer render (scroll/stream ticks) for up to renderCap blocks.
  const { text, images, toolRefs } = useMemo(() => {
    let text = ''
    let images: string[] = []
    let toolRefs: string[] = []
    if (typeof block.content === 'string') {
      const uri = extractDataUri(block.content)
      if (uri) images.push(uri)
      else text = block.content
    } else if (Array.isArray(block.content)) {
      const split = splitStructuredContent(block.content as ContentBlock[])
      text = split.text
      images = split.images
      toolRefs = split.toolRefs
    } else if (block.content != null) {
      text = JSON.stringify(block.content)
    } else {
      text = '(no content)'
    }
    return { text, images, toolRefs }
  }, [block.content])

  return (
    <>
      {toolRefs.length > 0 && (
        <div style="display:flex;flex-wrap:wrap;gap:4px;margin:4px 0">
          {toolRefs.map((name, i) => (
            <span
              key={i}
              style="padding:2px 8px;border-radius:var(--radius-xs);background:var(--tint-purple-medium);color:var(--accent-purple);font-size:10px;font-weight:600"
            >
              <i class="fas fa-plug" style="font-size:9px;margin-right:4px" />
              {name}
            </span>
          ))}
        </div>
      )}
      {images.map((src, i) => (
        <img
          key={i}
          src={src}
          alt="tool result"
          style={`max-width:100%;max-height:420px;margin:4px 0;border-radius:var(--radius-sm);display:block;border:1px solid ${block.is_error ? 'var(--accent-red)' : 'var(--bg-surface)'};cursor:pointer;`}
          onClick={(e) => {
            const img = e.currentTarget as HTMLImageElement
            img.style.maxHeight = img.style.maxHeight === 'none' ? '420px' : 'none'
          }}
        />
      ))}
      {text && (hasCliXmlTags(text)
        ? <CliXmlContent text={text} />
        : <JsonlToolResult content={text} isError={block.is_error} fullLen={block._fullLen} />)}
    </>
  )
}
