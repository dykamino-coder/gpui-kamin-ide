import type { JSX } from 'preact'
import { useState } from 'preact/hooks'
import { JsonlToolIcon } from './JsonlToolIcon'
import { formatTokens } from './utils'
import { EditToolRender } from './tool-renderers/EditToolRender'
import { WriteToolRender } from './tool-renderers/WriteToolRender'
import { TodoWriteRender } from './tool-renderers/TodoWriteRender'
import { BashToolRender, BashResultRender } from './tool-renderers/BashToolRender'
import { TaskCreateRender, TaskUpdateRender } from './tool-renderers/TaskRender'
import { AskUserQuestionRender } from './tool-renderers/AskUserQuestionRender'
import { AgentToolRender } from './tool-renderers/AgentToolRender'
import { GenericToolInput } from './tool-renderers/GenericToolInput'

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + '...' : s
}

/** Normalise MCP-prefixed tool names (`mcp__<server>__<tool>`) so the
 *  specialised renderers hit the Bash/Edit/Write/TodoWrite branches
 *  regardless of whether the tool came from CLI's built-ins or from our
 *  own `user-tools` MCP server. Returns both the short name and the
 *  MCP server tag so the header can still show provenance. */
function parseToolName(raw: string): { short: string; server?: string } {
  const m = raw.match(/^mcp__([^_]+(?:_[^_]+)*)__(.+)$/)
  if (m) return { short: m[2] ?? raw, server: m[1] }
  return { short: raw }
}

function summarizeToolInput(toolName: string, input: any): string {
  if (!input) return ''
  switch (toolName) {
    case 'Bash': return input.command ? truncate(input.command, 60) : ''
    case 'Read': return input.file_path || ''
    case 'Write': return input.file_path || ''
    case 'Edit': return input.file_path || ''
    case 'Glob': return input.pattern || ''
    case 'Grep': return input.pattern || ''
    case 'WebFetch': return input.url || ''
    case 'WebSearch': return input.query || ''
    case 'TeamCreate': return input.team_name || ''
    case 'TeamDelete': return input.team_name || ''
    case 'SendMessage': return `→ ${input.recipient || '?'}` + (input.type ? ` (${input.type})` : '')
    case 'Agent': return input.name || input.description || ''
    case 'TaskCreate': return input.subject ? truncate(input.subject, 60) : ''
    case 'TaskUpdate': return input.taskId ? `#${input.taskId} → ${input.status || '?'}` : ''
    default: {
      const keys = Object.keys(input)
      if (keys.length === 0) return ''
      const first = input[keys[0]]
      return typeof first === 'string' ? truncate(first, 50) : ''
    }
  }
}

interface JsonlToolUseProps {
  toolName: string
  toolUseId: string
  input: any
  result?: { content: string; isError: boolean }
}

/** Pick a tool-specific input renderer. Fallback = pretty JSON. Tools
 *  that get their own body (Edit/MultiEdit with diff, Write with
 *  content, TodoWrite with checklist, Bash with $-prompt) still share
 *  the collapse header + result chrome of JsonlToolUse. */
function renderToolBody(toolName: string, input: any): JSX.Element {
  switch (toolName) {
    case 'Edit':
    case 'MultiEdit':
      return <EditToolRender input={input} />
    case 'Write':
      return <WriteToolRender input={input} />
    case 'TodoWrite':
      return <TodoWriteRender input={input} />
    case 'Bash':
    case 'PowerShell':
      return <BashToolRender input={input} />
    case 'TaskCreate':
      return <TaskCreateRender input={input} />
    case 'TaskUpdate':
      return <TaskUpdateRender input={input} />
    case 'SendMessage': {
      // The message IS the point of a SendMessage (a teammate reporting its work,
      // the lead assigning a task) — show it as prose, not a JSON blob. This is
      // what makes a teammate's reasoning readable in the Agents panel: it sends
      // its answer via SendMessage, so without this the reader showed only a
      // collapsed tool chip.
      const content = typeof input?.content === 'string' ? input.content : ''
      return content
        ? <div class="result-body" style="white-space:pre-wrap">{content}</div>
        : <GenericToolInput input={input} />
    }
    // Исторический реплей вопроса (живой рисует ElicitationWidget).
    case 'AskUserQuestion':
      return <AskUserQuestionRender input={input} />
    case 'Agent':
      return <AgentToolRender input={input} />
    default:
      // Ключ-значение вместо сырого JSON — «неизвестных» блоков в чате
      // остаться не должно (аудит транскрипта 968683a8).
      return <GenericToolInput input={input} />
  }
}

function renderResultBody(toolName: string, result: { content: string; isError: boolean }): JSX.Element {
  if (toolName === 'Bash' || toolName === 'PowerShell') {
    return <BashResultRender text={result.content} isError={result.isError} />
  }
  return (
    <div class="result-body" style={result.isError ? 'color:var(--accent-red)' : undefined}>
      {result.content.length > 2000
        ? result.content.slice(0, 2000) + '\n... (truncated)'
        : result.content
      }
    </div>
  )
}

// Tools that stay EXPANDED by default — their whole value is the rendered
// body (a diff, a checklist) and the header summary can't convey it. Command
// tools (Bash/PowerShell) and Task* are collapsed by default: their one-line
// summary already shows the command/subject, so a closed block keeps the
// transcript tidy (the user opens the ones they care about).
const AUTO_EXPAND_TOOLS = new Set(['Edit', 'MultiEdit', 'Write', 'TodoWrite', 'SendMessage'])

export function JsonlToolUse({ toolName, toolUseId, input, result }: JsonlToolUseProps): JSX.Element {
  // Dispatch key = MCP-stripped short name. Falls through to the raw
  // name if there's no MCP prefix, so built-in tools work unchanged.
  const { short: shortName, server } = parseToolName(toolName)
  const [expanded, setExpanded] = useState(AUTO_EXPAND_TOOLS.has(shortName))
  const [resultExpanded, setResultExpanded] = useState(false)
  // ── Streaming-aware placeholders ─────────────────────────────────────
  // Live MITM-proxy stream: input arrives as `input_json_delta` chunks.
  // Until `content_block_stop` we cannot JSON.parse — so the synthesizer
  // marks the input with `{__streaming: true, __partial: '{"comm'}`.
  // Show a minimal placeholder; the moment the stop event fires, input
  // becomes the real parsed object and the normal renderer kicks in.
  if (input && typeof input === 'object' && input.__streaming === true) {
    return (
      <div class="block-tool-use streaming" data-tool-use-id={toolUseId}>
        <div class="tool-header">
          <span class="tool-icon" style="font-size:12px;flex-shrink:0">
            <JsonlToolIcon toolName={shortName} />
          </span>
          <span class="tool-name">{shortName}</span>
          <span class="tool-summary" style="font-style:italic;color:var(--text-muted)">
            <i class="fas fa-circle-notch fa-spin" style="margin-right:6px;font-size:10px" />
            preparing args…
          </span>
        </div>
      </div>
    )
  }
  if (input && typeof input === 'object' && input.__parseError === true) {
    return (
      <div class="block-tool-use" data-tool-use-id={toolUseId}>
        <div class="tool-header">
          <span class="tool-icon" style="font-size:12px;flex-shrink:0">
            <JsonlToolIcon toolName={shortName} />
          </span>
          <span class="tool-name">{shortName}</span>
          <span class="tool-summary" style="color:var(--accent-red)">
            tool args malformed
          </span>
        </div>
      </div>
    )
  }
  const summary = summarizeToolInput(shortName, input)
  const isSpecialized = shortName === 'Edit' || shortName === 'MultiEdit' || shortName === 'Write'
    || shortName === 'TodoWrite' || shortName === 'Bash' || shortName === 'PowerShell'
    || shortName === 'TaskCreate' || shortName === 'TaskUpdate'
    || shortName === 'AskUserQuestion' || shortName === 'Agent'

  return (
    <div
      class={`block-tool-use ${expanded ? 'expanded' : ''}`}
      data-tool-use-id={toolUseId}
    >
      <div class="tool-header" onClick={() => setExpanded(!expanded)}>
        <span class="tool-icon" style="font-size:12px;flex-shrink:0">
          <JsonlToolIcon toolName={shortName} />
        </span>
        <span class="tool-name">{shortName}</span>
        {server && (
          <span
            title={`Provided by MCP server "${server}"`}
            style="margin-left:6px;padding:1px 6px;border-radius:var(--radius-xs);background:var(--tint-purple-medium);color:var(--accent-purple);font-size:9px;font-weight:600;letter-spacing:0.03em;text-transform:uppercase;flex-shrink:0"
          >{server}</span>
        )}
        {summary && <span class="tool-summary">{summary}</span>}
        <span class="tool-arrow"><i class="fas fa-chevron-right" /></span>
      </div>
      <div class={isSpecialized ? 'tool-body tool-body-rich' : 'tool-body'} style={isSpecialized ? 'white-space:normal;padding:0 10px 6px' : undefined}>
        {renderToolBody(shortName, input)}
      </div>
      {result && (
        <div class={`tool-result-inline ${resultExpanded ? 'expanded' : ''}`}>
          <div
            class="result-header"
            onClick={() => setResultExpanded(!resultExpanded)}
          >
            <span class="result-arrow">
              <i class={result.isError ? 'fas fa-circle-xmark' : 'fas fa-chevron-right'} style={result.isError ? 'color:var(--accent-red)' : undefined} />
            </span>
            {' '}Result{' '}
            <span style={`color:${result.isError ? 'var(--accent-red)' : 'var(--text-disabled)'};font-size:10px`}>
              {formatTokens(result.content.length)} chars
            </span>
          </div>
          {resultExpanded && renderResultBody(shortName, result)}
        </div>
      )}
    </div>
  )
}
