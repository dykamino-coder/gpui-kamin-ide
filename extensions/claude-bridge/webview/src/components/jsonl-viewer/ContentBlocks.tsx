import type { JSX } from 'preact'
import type { ContentBlock } from '../../types/jsonl'
import { stripMcpPrefix } from '../../lib/mcp-tool-name'
import { JsonlTextBlock } from './JsonlTextBlock'
import { JsonlToolUse } from './JsonlToolUse'
import { ThinkingBlock } from './ThinkingBlock'
import { ToolResultBlock } from './ToolResultBlock'
import { ToolUseGroup } from './ToolUseGroup'
import { WidgetBlock } from './WidgetBlock'

/** ShowWidget renders as its own clean visual, never a tool chip — so it must
 *  never be folded into a tool group. */
function isWidgetBlock(block: ContentBlock | undefined): boolean {
  return !!block && block.type === 'tool_use' && stripMcpPrefix(block.name || '') === 'ShowWidget'
}

function renderToolUse(
  block: ContentBlock, key: string | number,
  toolResults?: Map<string, { content: string; isError: boolean }>,
): JSX.Element {
  return (
    <JsonlToolUse
      key={key}
      toolName={block.name || 'unknown'}
      toolUseId={block.id || ''}
      input={block.input}
      result={block.id ? toolResults?.get(block.id) : undefined}
    />
  )
}

export function renderContentBlocks(
  content: ContentBlock[],
  toolResults?: Map<string, { content: string; isError: boolean }>,
  streamingActiveBlock?: number,
): JSX.Element[] {
  // Streaming turns render blocks live/standalone (cursor + partial); only a
  // finalized message collapses parallel tool calls into a group.
  const streaming = streamingActiveBlock !== undefined
  const out: JSX.Element[] = []
  let i = 0
  while (i < content.length) {
    const block = content[i]
    // Defensive: a JSON array hole (old synthesizer skipped an unknown block)
    // deserialises to null. Skip rather than crash on `block.type`.
    if (!block) { out.push(<span key={i} />); i++; continue }
    if (block.type === 'tool_use') {
      // ShowWidget is a visual, not a tool chip — render it clean + standalone,
      // and never inside a tool group.
      if (isWidgetBlock(block)) {
        out.push(<WidgetBlock key={i} input={block.input} />)
        i++
        continue
      }
      // Collapse a run of ≥2 consecutive tool_use blocks into one "Read ×3"
      // group, mirroring the entry-level tool burst. The run stops at a widget
      // so it never gets swallowed.
      let j = i
      while (j < content.length && content[j]?.type === 'tool_use' && !isWidgetBlock(content[j])) j++
      const run = content.slice(i, j)
      if (!streaming && run.length >= 2) {
        out.push(<ToolUseGroup key={`tg-${i}`} blocks={run} toolResults={toolResults} />)
      } else {
        for (let k = i; k < j; k++) out.push(renderToolUse(content[k]!, k, toolResults))
      }
      i = j
      continue
    }
    switch (block.type) {
      case 'thinking':
        out.push(<ThinkingBlock key={i} thinking={block.thinking || ''} />)
        break
      case 'text':
        out.push(<JsonlTextBlock key={i} text={block.text || ''} isStreaming={streamingActiveBlock === i} />)
        break
      case 'tool_result':
        out.push(<ToolResultBlock key={i} block={block} />)
        break
      default: {
        // A block type we don't explicitly render — e.g. a NEW Anthropic type
        // (server_tool_use, web_search_tool_result, mcp_tool_use/result,
        // redacted_thinking). Route by shape so it still renders with the right
        // chrome; never drop it to an empty span (silent content loss).
        const b = block as unknown as { type?: string; name?: string; id?: string; input?: unknown; content?: unknown; text?: string; thinking?: string }
        if (b.input !== undefined && (b.name || b.id)) {
          out.push(renderToolUse(block, i, toolResults)) // tool_use-shaped (server_tool_use, mcp_tool_use)
        } else if (b.content !== undefined) {
          out.push(<ToolResultBlock key={i} block={block} />) // *_tool_result-shaped
        } else if (b.type === 'redacted_thinking') {
          out.push(<div key={i} class="unknown-block">🔒 redacted thinking</div>)
        } else if (b.type) {
          const preview = (b.text ?? b.thinking ?? JSON.stringify(b)).slice(0, 4000)
          out.push(<div key={i} class="unknown-block">[block: {b.type}] {preview}</div>)
        } else {
          out.push(<span key={i} />)
        }
      }
    }
    i++
  }
  return out
}
