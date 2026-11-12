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
  return text.replace(/[\x00-\x08\x0b-\x1f\x7f\x9b]/g, '')
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
    `${BOLD}${CYAN}ChatGPT (codex)${RESET} ${info.resumed ? 'resumed' : 'started'} thread ${info.threadId}`,
    `${DIM}model ${info.model ?? 'default'}${info.cwd ? ` · ${info.cwd}` : ''}${RESET}`,
    `${DIM}This console shows the thread's live events; the chat holds the full conversation.${RESET}`,
    '',
  ]
  return toTerminal(lines.join('\n') + '\n')
}

/** One message line for Bridge's own notices (restart, stall, sign-in). */
export function consoleNotice(level: 'warning' | 'error', message: string, at = Date.now()): string {
  const color = level === 'error' ? RED : YELLOW
  return toTerminal(`${DIM}${clock(at)}${RESET} ${color}${level}${RESET} ${plainText(message)}\n`)
}

/** Turns normalized events into console text; keeps which replies streamed. */
export class CodexConsoleRenderer {
  private streaming: string | null = null
  private readonly streamed = new Set<string>()

  render(event: AgentEvent): string {
    const t = `${DIM}${clock(event.at)}${RESET} `
    switch (event.kind) {
      case 'session.started':
        return ''
      case 'turn.started':
        return this.close() + toTerminal(`${t}${DIM}── turn started${RESET}\n`)
      case 'turn.completed': {
        const took = event.durationMs !== undefined ? ` in ${(event.durationMs / 1000).toFixed(1)} s` : ''
        const color = event.status === 'completed' ? GREEN : event.status === 'interrupted' ? YELLOW : RED
        const error = event.error ? `: ${plainText(event.error.message)}` : ''
        return this.close() + toTerminal(`${t}${color}── turn ${event.status}${took}${error}${RESET}\n`)
      }
      case 'message.user':
        return this.close() + toTerminal(`${t}${BOLD}you${RESET} ${preview(event.text)}\n`)
      case 'message.assistant.delta': {
        let head = ''
        if (this.streaming !== event.id) {
          head = this.close() + `${t}${BOLD}${MAGENTA}assistant${RESET} `
          this.streaming = event.id
          this.streamed.add(event.id)
        }
        return head + toTerminal(plainText(event.delta))
      }
      case 'message.assistant': {
        if (this.streamed.delete(event.id)) {
          if (this.streaming === event.id) return this.close()
          return ''
        }
        return this.close() + toTerminal(`${t}${BOLD}${MAGENTA}assistant${RESET} ${plainText(event.text)}\n`)
      }
      case 'reasoning':
        return event.summary.length
          ? this.close() + toTerminal(`${t}${DIM}thinking: ${preview(event.summary.join(' '), 200)}${RESET}\n`)
          : ''
      case 'tool.started':
        return (
          this.close() +
          toTerminal(
            `${t}${CYAN}→ ${toolName(event.tool)}${RESET} ${DIM}${preview(JSON.stringify(event.input ?? {}), 200)}${RESET}\n`,
          )
        )
      case 'tool.completed': {
        const color = event.status === 'completed' ? GREEN : RED
        const took = event.durationMs !== undefined ? ` ${(event.durationMs / 1000).toFixed(1)} s` : ''
        const body = event.error ?? outputText(event.output)
        return (
          this.close() +
          toTerminal(
            `${t}${color}← ${toolName(event.tool)} ${event.status}${took}${RESET}${body ? ` ${DIM}${preview(body)}${RESET}` : ''}\n`,
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
        return this.close() + consoleNotice(event.level, event.message, event.at)
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
