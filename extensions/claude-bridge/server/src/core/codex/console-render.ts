// ============================================================================
// Bridge Console text for a codex session.
//
// A Claude session's console is the CLI's own terminal. A codex thread has no
// terminal, so its virtual PTY prints this live log of the thread instead:
// turns, the user's and the assistant's text as it streams, tool calls with a
// short result, model changes, usage and errors. The chat stays the full
// transcript; the console is the operator's view of what the engine is doing.
// ============================================================================

import type { AgentEvent, AgentToolOutput, AgentToolRef } from '../../shared/agent-events'

const RESET = '\x1b[0m'
const DIM = '\x1b[2m'
const BOLD = '\x1b[1m'
const RED = '\x1b[31m'
const GREEN = '\x1b[32m'
const YELLOW = '\x1b[33m'
const CYAN = '\x1b[36m'
const MAGENTA = '\x1b[35m'

const PREVIEW_CHARS = 400

/** xterm needs CRLF; lone LFs only move the cursor down. */
export function toTerminal(text: string): string {
  return text.replace(/\r?\n/g, '\r\n')
}

/** Engine text never drives the terminal: control bytes (ESC, OSC titles,
 *  BEL, CR) are dropped, only newlines and tabs stay. */
export function plainText(text: string): string {
  return text.replace(/[\x00-\x08\x0b-\x1f\x7f-\x9f]/g, '')
}

function preview(text: string, limit = PREVIEW_CHARS): string {
  const flat = plainText(text).trim()
  return flat.length > limit ? `${flat.slice(0, limit)}… (${flat.length} chars)` : flat
}

function toolName(tool: AgentToolRef): string {
  return plainText(tool.server ? `${tool.server}.${tool.name}` : tool.name)
}

function outputText(output: AgentToolOutput[]): string {
  return output
    .map((o) => (o.type === 'text' ? o.text : o.type === 'image' ? '[image]' : JSON.stringify(o.value)))
    .join('\n')
}

function clock(at: number): string {
  return new Date(at).toISOString().slice(11, 19)
}

/** First lines a console shows for the thread (also after a reattach). */
export function consoleBanner(info: {
  threadId: string
  model: string | null
  resumed: boolean
  cwd?: string
}): string {
  const lines = [
    `${BOLD}${CYAN}ChatGPT (codex)${RESET} ${info.resumed ? 'resumed' : 'started'} thread ${plainText(info.threadId)}`,
    `${DIM}model ${plainText(info.model ?? 'default')}${info.cwd ? ` · ${plainText(info.cwd)}` : ''}${RESET}`,
    `${DIM}This console shows the thread's live events; the chat holds the full conversation.${RESET}`,
    '',
  ]
  return toTerminal(lines.join('\n') + '\n')
}

/** One message line for Bridge's own notices (restart, stall, sign-in). */
export function consoleNotice(level: 'info' | 'warning' | 'error', message: string, at = Date.now()): string {
  const color = level === 'error' ? RED : level === 'warning' ? YELLOW : DIM
  return toTerminal(`${DIM}${clock(at)}${RESET} ${color}${level}${RESET} ${plainText(message)}\n`)
}

/** Continuation lines stay under the role header, including streamed newlines. */
function body(text: string): string {
  return toTerminal(plainText(text)).replaceAll('\r\n', '\r\n  ')
}

function heading(mark: string, label: string, style: string, at: number): string {
  return `\r\n${style}${mark} ${label}${RESET}  ${DIM}${clock(at)}${RESET}\r\n  `
}

/** Turns normalized events into console text; keeps which replies streamed. */
export class CodexConsoleRenderer {
  private streaming: string | null = null
  private readonly streamed = new Map<string, string>()
  // Only public summary parts enter the console. An item may finish with
  // additional parts or a suffix the app-server never streamed.
  private readonly summaries = new Map<string, Map<number, { raw: string; pending: string; heading: boolean }>>()

  notice(level: 'info' | 'warning' | 'error', message: string, at = Date.now()): string {
    return this.close() + consoleNotice(level, message, at)
  }

  private summary(id: string, index: number, delta: string, at: number, final = false): string {
    const parts = this.summaries.get(id) ?? new Map<number, { raw: string; pending: string; heading: boolean }>()
    const part = parts.get(index) ?? { raw: '', pending: '', heading: true }
    if (final && delta.startsWith(part.raw)) delta = delta.slice(part.raw.length)
    if (!delta && !part.pending) return ''
    part.raw += delta
    parts.set(index, part)
    this.summaries.set(id, parts)
    const key = `summary:${id}:${index}`
    let out = ''
    if (this.streaming !== key) {
      const nextPart = index > 0 && this.streaming === `summary:${id}:${index - 1}`
      out = this.close() + (nextPart ? '\r\n  ' : heading('✦', 'Thinking', CYAN, at))
      this.streaming = key
    }
    // The CLI's public summaries often start with a Markdown heading whose
    // opening or closing delimiters span deltas. Only style a complete heading.
    let content = part.pending + delta
    part.pending = ''
    if (part.heading) {
      const end = content.indexOf('**', 2)
      if (content.startsWith('**') && end > 2 && !content.slice(2, end).includes('\n')) {
        out += BOLD + body(content.slice(2, end)) + RESET
        content = content.slice(end + 2)
      } else if (
        !final &&
        (content === '*' || content.startsWith('**')) &&
        !content.includes('\n') &&
        content.length < 256
      ) {
        part.pending = content
        return out
      }
      part.heading = false
    }
    return out + (content ? DIM + body(content) + RESET : '')
  }

  render(event: AgentEvent): string {
    const t = `${DIM}${clock(event.at)}${RESET} `
    switch (event.kind) {
      case 'session.started':
        return ''
      case 'turn.started':
        return this.close() + toTerminal(`${t}${DIM}── turn started${RESET}\n`)
      case 'turn.requestAccepted':
        return (
          this.close() +
          toTerminal(
            `${t}${DIM}turn/start accepted · requested effort ${plainText(event.requestedEffort ?? 'omitted')}${RESET}\n`,
          )
        )
      case 'turn.completed': {
        this.streamed.clear()
        this.summaries.clear()
        const took = event.durationMs !== undefined ? ` in ${(event.durationMs / 1000).toFixed(1)} s` : ''
        const color = event.status === 'completed' ? GREEN : event.status === 'interrupted' ? YELLOW : RED
        const error = event.error ? `: ${plainText(event.error.message)}` : ''
        return this.close() + toTerminal(`${t}${color}── turn ${event.status}${took}${error}${RESET}\n`)
      }
      case 'message.user':
        return this.close() + heading('❯', 'You', BOLD, event.at) + body(event.text) + '\r\n'
      case 'message.assistant.delta': {
        let head = ''
        if (this.streaming !== 'message:' + event.id) {
          head = this.close() + heading('●', 'ChatGPT', BOLD + MAGENTA, event.at)
          this.streaming = 'message:' + event.id
        }
        this.streamed.set(event.id, (this.streamed.get(event.id) ?? '') + event.delta)
        return head + body(event.delta)
      }
      case 'message.assistant': {
        const streamed = this.streamed.get(event.id)
        this.streamed.delete(event.id)
        const tail = streamed && event.text.startsWith(streamed) ? event.text.slice(streamed.length) : event.text
        if (!tail) return this.streaming === 'message:' + event.id ? this.close() : ''
        const header =
          this.streaming === 'message:' + event.id
            ? ''
            : this.close() + heading('●', 'ChatGPT', BOLD + MAGENTA, event.at)
        this.streaming = null
        return header + body(tail) + '\r\n'
      }
      case 'reasoning.delta':
        return event.channel === 'summary' ? this.summary(event.id, event.index, event.delta, event.at) : ''
      case 'reasoning': {
        let out = ''
        for (const [index, text] of event.summary.entries()) {
          out += this.summary(event.id, index, text, event.at, true)
        }
        for (const [index, part] of this.summaries.get(event.id) ?? []) {
          if (part.pending) out += this.summary(event.id, index, '', event.at, true)
        }
        this.summaries.delete(event.id)
        return out + (this.streaming?.startsWith(`summary:${event.id}:`) ? this.close() : '')
      }
      case 'tool.started':
        return (
          this.close() +
          toTerminal(
            `${t}${CYAN}→ ${toolName(event.tool)}${RESET}\n  ${DIM}${body(preview(JSON.stringify(event.input ?? {}), 200))}${RESET}\n`,
          )
        )
      case 'tool.completed': {
        const color = event.status === 'completed' ? GREEN : RED
        const took = event.durationMs !== undefined ? ` ${(event.durationMs / 1000).toFixed(1)} s` : ''
        const body = event.error ?? outputText(event.output)
        return (
          this.close() +
          toTerminal(
            `${t}${color}← ${toolName(event.tool)} ${event.status}${took}${RESET}${body ? `\n  ${DIM}${preview(body).replaceAll('\n', '\n  ')}${RESET}` : ''}\n`,
          )
        )
      }
      case 'context.compaction':
        return this.close() + toTerminal(`${t}${DIM}context compacted${RESET}\n`)
      case 'usage':
        return ''
      case 'model.rerouted':
        return (
          this.close() +
          toTerminal(
            `${t}${YELLOW}model ${plainText(`${event.from} → ${event.to}`)}${RESET} ${DIM}${plainText(event.reason)}${RESET}\n`,
          )
        )
      case 'notice':
        return this.notice(event.level, event.message, event.at)
      case 'item.unsupported':
        return this.close() + toTerminal(`${t}${DIM}(${plainText(event.itemType)} item not shown)${RESET}\n`)
      case 'session.exited':
        return (
          this.close() +
          toTerminal(`${t}${DIM}codex thread ended${event.reason ? `: ${plainText(event.reason)}` : ''}${RESET}\n`)
        )
      default:
        return ''
    }
  }

  /** Ends a streaming reply line before anything else is printed. */
  private close(): string {
    if (this.streaming === null) return ''
    this.streaming = null
    return '\r\n'
  }
}
