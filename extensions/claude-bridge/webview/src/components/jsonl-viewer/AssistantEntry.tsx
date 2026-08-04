import type { JSX } from 'preact'
import type { JsonlEntryData, ContentBlock } from '../../types/jsonl'
import { formatTokens, hasCliXmlTags } from './utils'
import { getAgentColor } from '../../utils/agent-color'
import { activeSessionTitle } from '../../signals/tabs'
import { serverPathVisible } from '../../signals/ui'
import { JsonlTimestamp } from './JsonlTimestamp'
import { CliXmlContent } from './CliXmlContent'
import { ToolResultBlock } from './ToolResultBlock'
import { JsonlTextBlock } from './JsonlTextBlock'
import { renderContentBlocks } from './ContentBlocks'
import { MessageActions } from './MessageActions'

interface AssistantEntryProps {
  entry: JsonlEntryData
  toolResults?: Map<string, { content: string; isError: boolean }>
}

/** Classify an assistant text payload into who caused the error — Anthropic
 *  API (upstream), Claude Code CLI (local wrapper), Open Claude Bridge (us),
 *  or a regular message. Strings are deliberately narrow so we don't
 *  misattribute normal output. */
function classifyAssistantText(text: string): { origin: 'anthropic' | 'cli' | 'bridge' | null; label: string } {
  // Anthropic API failure — CLI prefixes with "API Error:" and often
  // includes the node-fetch hint "pass verbose: true in the second
  // argument to fetch()". These never come from our code.
  if (/\bAPI Error:\b/.test(text) && (/\bfetch\(\)/.test(text) || /socket connection|ECONNRESET|timed out|overloaded|rate_limit/i.test(text))) {
    return { origin: 'anthropic', label: 'Anthropic API error · not Open Claude Bridge' }
  }
  // Plain "API Error:" without fetch hint — likely still upstream but mark carefully.
  if (/^API Error:/m.test(text)) {
    return { origin: 'anthropic', label: 'Upstream API error (Claude / Anthropic)' }
  }
  // CLI-level failures usually carry "Error: " + one of these CLI-specific keywords.
  if (/\b(prompt is too long|context.{0,10}window|maximum.{0,10}context)/i.test(text)) {
    return { origin: 'cli', label: 'Claude Code CLI error' }
  }
  // Our own bridge errors go through the MCP tool_result path, not assistant
  // text, so if the user ever sees `[open-claude-bridge]` or a specific tool
  // failure from our HTTP proxy, mark it as ours.
  if (/\b(open.?claude.?bridge|bridge MCP)\b/i.test(text) && /error|failed|timeout/i.test(text)) {
    return { origin: 'bridge', label: 'Open Claude Bridge error' }
  }
  return { origin: null, label: '' }
}

export function AssistantEntry({ entry, toolResults }: AssistantEntryProps): JSX.Element | null {
  const usage = entry.usage || entry.message?.usage
  const content = entry.message?.content

  // Skip if content has nothing visible to render (empty array, only empty
  // text/thinking blocks). Applies to streaming entries too — between
  // message_start and the first content_block_delta we'd otherwise show a
  // bare assistant header on empty space, which the user reads as a stuck
  // "block with no body". Better to delay the bubble appearance until the
  // first byte of real content arrives; the activity spinner under the
  // input bar already covers that gap.
  if (Array.isArray(content)) {
    const hasVisible = (content as ContentBlock[]).some(b => {
      if (!b || !(b as { type?: string }).type) return false
      if (b.type === 'thinking') return !!(b.thinking && b.thinking.trim())
      if (b.type === 'text') return !!(b.text && b.text.trim())
      // tool_use, tool_result, and any NEW/unknown Anthropic block type render
      // something (ContentBlocks routes unknowns to a labelled fallback) — so a
      // message that only carries such a block must not be skipped entirely.
      return true
    })
    if (!hasVisible) return null
  }

  // Origin classifier — inspect all visible text to tag the entry header.
  let textForClassify = ''
  if (typeof content === 'string') textForClassify = content
  else if (Array.isArray(content)) {
    for (const b of content as ContentBlock[]) {
      if (b.type === 'text' && b.text) textForClassify += b.text + '\n'
    }
  }
  // Живой стрим НЕ классифицируем: 5 регэкспов по растущему ответу гонялись
  // на каждый rAF-кадр хвоста (аудит #70 B7); ошибки метятся на каноникале,
  // который watcher доставляет сразу после стрима.
  const classified = entry.__streaming === undefined
    ? classifyAssistantText(textForClassify)
    : null

  const tokenParts: string[] = []
  if (usage?.input_tokens) tokenParts.push(`in ${formatTokens(usage.input_tokens)}`)
  // Cache-hit tokens are typically 10× cheaper than regular input
  // (modelCost.ts in CLI: $0.30 vs $3/Mtok on Sonnet tier). Surfacing
  // them separately lets users see why a long session is still cheap —
  // without it, the `in: 120K` number looked like $0.36 every turn when
  // in reality most of it was $0.036 cache re-reads.
  const cacheRead = (usage as any)?.cache_read_input_tokens as number | undefined
  const cacheWrite = (usage as any)?.cache_creation_input_tokens as number | undefined
  if (cacheRead) tokenParts.push(`cached ${formatTokens(cacheRead)}`)
  if (cacheWrite) tokenParts.push(`+cache ${formatTokens(cacheWrite)}`)
  if (usage?.output_tokens) tokenParts.push(`out ${formatTokens(usage.output_tokens)}`)

  // System-info visibility — same toggle that gates Server path / IDs row
  // up in ChatHeader. When off, hide the assistant's model name + token
  // breakdown so the bubble header stays clean.
  const systemInfoVisible = serverPathVisible.value

  // Color based on session title hash
  // Узкий computed вместо широкого tabs: запись tabs (флап соединения)
  // ререндерила все баблы ленты (аудит #70 D3).
  const agentColor = getAgentColor(activeSessionTitle.value)
  const roleColor = agentColor || 'var(--accent-green)'
  // Propagate the agent accent through a CSS variable so nested `.tool-name`
  // chips match the same pink/red/green hue the session's header uses.
  const cardStyle = `border-left-color:${roleColor};--agent-color:${roleColor}`

  return (
    <div class="jsonl-entry jsonl-entry-assistant" style={cardStyle}>
      <div class="entry-header">
        <span class="entry-role" style={`color:${roleColor}`}>
          <span class="entry-role-label">
            <span class="entry-role-dot" style={`background:${roleColor}`} aria-hidden="true" />
            {entry.isSidechain
              ? <><i class="fas fa-robot" style="font-size:10px" />{' '}Assistant</>
              : 'Assistant'}
          </span>
          {' '}
          {entry.timestamp && <JsonlTimestamp timestamp={entry.timestamp} />}
        </span>
        {entry.isSidechain && (
          <span class="entry-sidechain">Subagent</span>
        )}
        {systemInfoVisible && entry.model && (
          <span class="entry-model">{entry.model}</span>
        )}
        {systemInfoVisible && tokenParts.length > 0 && (
          <span class="entry-tokens">{tokenParts.join(' | ')}</span>
        )}
        {classified?.origin && (
          <span
            data-tooltip={classified.label}
            style={`margin-left:8px;padding:1px 8px;border-radius:var(--radius-xs);font-size:10px;letter-spacing:0.3px;border:1px solid ${classified.origin === 'anthropic' ? 'var(--accent-red)' : classified.origin === 'cli' ? 'var(--accent-orange)' : 'var(--accent-primary)'};color:${classified.origin === 'anthropic' ? 'var(--accent-red)' : classified.origin === 'cli' ? 'var(--accent-orange)' : 'var(--accent-primary)'};background:var(--bg-mantle)`}
          >
            <i class={classified.origin === 'anthropic' ? 'fas fa-cloud-bolt' : classified.origin === 'cli' ? 'fas fa-terminal' : 'fas fa-plug'} style="margin-right:4px" />
            {classified.origin === 'anthropic' ? 'Anthropic API' : classified.origin === 'cli' ? 'Claude CLI' : 'Open Claude Bridge'}
          </span>
        )}
        <MessageActions
          text={textForClassify.trim() || (typeof content === 'string' ? content : JSON.stringify(content, null, 2))}
          defaultName={`assistant-${entry.timestamp ?? Date.now()}.md`}
        />
      </div>
      {typeof content === 'string'
        ? <JsonlTextBlock text={content} />
        : Array.isArray(content)
          ? renderContentBlocks(
              content,
              toolResults,
              entry.__streaming === true ? entry.__streamingActiveBlock : undefined,
            )
          : null
      }
    </div>
  )
}

export function ToolResultUserEntry({ entry }: { entry: JsonlEntryData }): JSX.Element {
  const content = entry.message?.content
  if (!Array.isArray(content)) return <div class="jsonl-entry jsonl-entry-assistant" />

  return (
    <div class="jsonl-entry jsonl-entry-assistant">
      {content.map((block: ContentBlock, i: number) => {
        if (block.type === 'tool_result') {
          return <ToolResultBlock key={i} block={block} />
        }
        if (block.type === 'text' && block.text) {
          if (hasCliXmlTags(block.text)) {
            return <CliXmlContent key={i} text={block.text} />
          }
          if (block.text.trim()) {
            return (
              <div key={i} style="font-size:12px;color:var(--text-subtext);padding:2px 0;">
                {block.text}
              </div>
            )
          }
        }
        return null
      })}
    </div>
  )
}
