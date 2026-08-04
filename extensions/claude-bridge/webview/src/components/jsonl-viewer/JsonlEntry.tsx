import type { JSX } from 'preact'
import type { JsonlEntryData } from '../../types/jsonl'
import { isToolResultEntry, isCaveatOnlyEntry, isTeammateMessageEntry, NON_RENDERING_ENTRY_TYPES } from './utils'
import { UserEntry } from './UserEntry'
import { SystemEntry } from './SystemEntry'
import { AssistantEntry, ToolResultUserEntry } from './AssistantEntry'
import { TeammateEntry } from './TeammateEntry'
import { CompactSummaryEntry } from './CompactSummaryEntry'
import { ToolGroupEntry } from './ToolGroupEntry'

interface JsonlEntryProps {
  entry: JsonlEntryData
  toolResults?: Map<string, { content: string; isError: boolean }>
}

export function JsonlEntry({ entry, toolResults }: JsonlEntryProps): JSX.Element | null {
  // Session-level bookkeeping the CLI writes but which isn't a message
  // (last-prompt, permission-mode, queue-operation, file-history-snapshot,
  // ai-title, custom-title, agent-name, summary). Same list `entryIsVisible`
  // filters on — they must never reach here, but returning null keeps the two
  // honest if one ever slips through.
  if (NON_RENDERING_ENTRY_TYPES.has(entry.type)) return null
  if (entry.type === 'tool_group') return <ToolGroupEntry entry={entry} toolResults={toolResults} />

  // CLI 2.1.116+ writes several message kinds as `type:"attachment"` with a
  // nested `attachment.type`. Most are housekeeping that only the model sees
  // (deferred tool lists, MCP instruction deltas, skill listings, empty
  // task_reminder), but two are user-visible: queued prompts and external
  // file edits. Render those explicitly; silently drop the rest.
  if (entry.type === 'attachment') {
    const att = entry.attachment
    if (att?.type === 'queued_command' && typeof att.prompt === 'string' && att.prompt.trim()) {
      const wrapped: JsonlEntryData = {
        ...entry,
        type: 'user',
        message: { role: 'user', content: att.prompt },
      } as any
      return <UserEntry entry={wrapped} />
    }
    if (att?.type === 'edited_text_file' && typeof att.filename === 'string') {
      return (
        <div class="jsonl-entry jsonl-entry-system">
          <div class="entry-role">
            <i class="fas fa-pen-to-square" style="font-size:10px;color:var(--accent-purple)" />
            {' '}
            <span style="color:var(--accent-purple)">External edit</span>
            {' '}
            <span style="color:var(--text-muted);font-size:11px;margin-left:6px;font-family:monospace">{att.filename}</span>
          </div>
          {typeof att.snippet === 'string' && att.snippet.trim() && (
            <pre style="margin:6px 0 0;padding:8px 12px;background:var(--bg-mantle);border:1px solid var(--bg-surface);border-radius:var(--radius-sm);font-size:11px;color:var(--text-secondary);font-family:ui-monospace,Consolas,monospace;overflow-x:auto;max-height:180px;overflow-y:auto;line-height:1.5">{att.snippet}</pre>
          )}
        </div>
      )
    }
    return null
  }

  if (entry.type === 'user') {
    // Compact summary entries carry the long summary produced by `/compact`.
    // Render them as a collapsible block rather than hiding or showing as user.
    if (entry.isCompactSummary) return <CompactSummaryEntry entry={entry} />
    // `isMeta: true` marks CLI-generated notices like
    // "Compacted (ctrl+o to see full summary)" or scheduled-task stubs —
    // not from the human user, so hide from the chat log.
    if (entry.isMeta) return null
    // Some CLIs forget the flag; match the well-known literal as a safety net.
    // Content may be either a string or an array of ContentBlocks, so extract
    // the joined text before testing. Also strip any wrapper XML so the match
    // still fires when the CLI decorates the caveat with `<local-command-*>`.
    const rawContent = entry.message?.content
    let rawText = ''
    if (typeof rawContent === 'string') rawText = rawContent
    else if (Array.isArray(rawContent)) {
      rawText = (rawContent as any[])
        .filter(b => b && b.type === 'text' && typeof b.text === 'string')
        .map(b => b.text)
        .join('\n')
    }
    const stripped = rawText
      .replace(/<[^>]+>/g, ' ')
      .replace(/\s+/g, ' ')
      .trim()
    if (/compacted\s*\(ctrl[\+\s]*o\s+to\s+see\s+full\s+summary\)/i.test(stripped)) return null
    if (isToolResultEntry(entry)) {
      // Skip if all tool_results in this entry were merged into assistant blocks
      if (toolResults && toolResults.size > 0) {
        const content = entry.message?.content
        if (Array.isArray(content)) {
          const allMerged = content.every((b: any) =>
            b.type !== 'tool_result' || (b.tool_use_id && toolResults.has(b.tool_use_id))
          )
          if (allMerged) return null
        }
      }
      return <ToolResultUserEntry entry={entry} />
    }
    if (isCaveatOnlyEntry(entry)) return null
    if (isTeammateMessageEntry(entry)) return <TeammateEntry entry={entry} />
    // CLI rejects an unrecognised slash command by injecting a user-typed
    // `Unknown command: /foo` into the transcript (we saw this e.g. when a
    // plugin command was renamed). It's not something the human said, and
    // the blue USER bubble makes it look like the user typed it — style as
    // a muted system/warning block instead.
    if (rawText.trim().startsWith('Unknown command:')) {
      return (
        <div class="jsonl-entry jsonl-entry-system">
          <div class="entry-role">
            <i class="fas fa-circle-exclamation" style="font-size:10px;color:var(--accent-yellow)" />
            {' '}
            <span style="color:var(--accent-yellow);font-weight:600">Unknown command</span>
            {' '}
            {entry.timestamp && <span style="color:var(--text-muted);font-size:10px">{entry.timestamp}</span>}
          </div>
          <div style="padding:6px 12px;background:var(--tint-yellow-soft);border:1px solid var(--tint-yellow-border);border-radius:var(--radius-sm);margin-top:4px;color:var(--text-subtext);font-size:12px;font-family:ui-monospace,Consolas,monospace">
            {rawText.trim()}
          </div>
        </div>
      )
    }
    return <UserEntry entry={entry} />
  }

  if (entry.type === 'assistant') return <AssistantEntry entry={entry} toolResults={toolResults} />
  if (entry.type === 'system') return <SystemEntry entry={entry} />

  return null
}
