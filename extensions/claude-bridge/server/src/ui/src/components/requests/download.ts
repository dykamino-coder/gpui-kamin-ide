import type { RequestLogEntry } from '../../types'
import { formatTokens as fmtTokens, formatFullDate } from '../../utils/formatters'

export function downloadRequest(entry: RequestLogEntry) {
  const sep = '='.repeat(60)
  const sections: string[] = []

  sections.push(sep)
  sections.push(`CAPTURED REQUEST — ${entry.id}`)
  sections.push(sep)
  sections.push(`Time:      ${entry.timestamp}`)
  sections.push(`Plugin:    ${entry.pluginId}`)
  if (entry.sessionKey) sections.push(`Session:   ${entry.sessionKey}`)
  sections.push(`Endpoint:  ${entry.endpoint}`)
  sections.push(`Model:     ${entry.model}`)
  sections.push(`Status:    ${entry.status} (${entry.statusCode})`)
  sections.push(`Duration:  ${entry.durationMs}ms`)
  const totalIn = entry.inputTokens + (entry.cacheReadTokens || 0) + (entry.cacheWriteTokens || 0)
  sections.push(`Tokens:    ${totalIn} in / ${entry.outputTokens} out`)
  if (entry.cacheReadTokens > 0) sections.push(`  Cache read:   ${entry.cacheReadTokens}`)
  if (entry.cacheWriteTokens > 0) sections.push(`  Cache write:  ${entry.cacheWriteTokens}`)
  sections.push(`  Raw input:    ${entry.inputTokens}`)
  if (entry.toolsUsed.length > 0) sections.push(`Tools:     ${entry.toolsUsed.join(', ')}`)
  if (entry.error) sections.push(`Error:     ${entry.error}`)

  if (entry.requestBody) {
    sections.push('')
    sections.push(sep)
    sections.push('REQUEST BODY')
    sections.push(sep)
    sections.push(JSON.stringify(entry.requestBody, null, 2))
  }

  if (entry.responseText) {
    sections.push('')
    sections.push(sep)
    sections.push('RESPONSE')
    sections.push(sep)
    sections.push(entry.responseText)
  }

  const blob = new Blob([sections.join('\n')], { type: 'text/plain' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `request-${entry.id}.txt`
  a.click()
  URL.revokeObjectURL(url)
}

export function hasCapture(entry: RequestLogEntry): boolean {
  return !!(entry.requestBody || entry.responseText)
}

export function downloadRequestHtml(entry: RequestLogEntry) {
  const time = formatFullDate(entry.timestamp)
  const body: any = entry.requestBody || {}
  const messagesHtml = renderMessages(body, entry)
  const systemHtml = renderSystem(body)
  const toolDefsHtml = renderToolDefs(body)
  const requestParamsHtml = renderRequestParams(body)

  const totalIn = entry.inputTokens + (entry.cacheReadTokens || 0) + (entry.cacheWriteTokens || 0)
  const metaParts = [
    entry.pluginId,
    entry.endpoint.toUpperCase(),
    entry.model,
    `${entry.status} (${entry.statusCode})`,
    entry.durationMs > 0 ? `${entry.durationMs}ms` : null,
    `${fmtTokens(totalIn)} in / ${fmtTokens(entry.outputTokens)} out`,
  ].filter(Boolean)

  const errorHtml = entry.error
    ? `<div class="error-box">⚠ ${esc(entry.error)}</div>`
    : ''

  const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Request ${esc(String(entry.id))}</title>
<style>
  *{margin:0;padding:0;box-sizing:border-box}
  body{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:#0d1117;color:#c9d1d9;line-height:1.6}
  code,pre{font-family:'JetBrains Mono','Fira Code','Cascadia Code',monospace}
  /* header */
  .header{background:#161b22;border-bottom:1px solid #30363d;padding:20px 24px}
  .header h1{font-size:16px;font-weight:600;color:#f0f6fc;margin-bottom:4px}
  .meta-row{display:flex;flex-wrap:wrap;gap:8px;font-size:12px}
  .meta-tag{background:#1c2128;border:1px solid #30363d;border-radius:4px;padding:2px 8px;color:#8b949e}
  .meta-tag.model{color:#d2a8ff;border-color:#d2a8ff30}
  .meta-tag.plugin{color:#58a6ff;border-color:#58a6ff30}
  .meta-tag.tokens{color:#3fb950;border-color:#3fb95030}
  .meta-tag.time{color:#d29922;border-color:#d2992230}
  .meta-tag.status-ok{color:#3fb950;border-color:#3fb95030}
  .meta-tag.status-err{color:#f85149;border-color:#f8514930}
  /* content */
  .content{max-width:960px;margin:0 auto;padding:24px 16px;display:flex;flex-direction:column;gap:12px}
  .error-box{background:rgba(248,81,73,0.1);border:1px solid rgba(248,81,73,0.3);border-radius:8px;padding:12px 16px;font-size:13px;color:#f85149}
  /* messages */
  .msg{display:flex;gap:12px;max-width:90%}
  .msg.user{align-self:flex-end;flex-direction:row-reverse}
  .msg.assistant{align-self:flex-start}
  .msg.system-msg{align-self:center;max-width:100%}
  .avatar{width:32px;height:32px;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:12px;font-weight:700;flex-shrink:0}
  .msg.user .avatar{background:#1f6feb;color:#fff}
  .msg.assistant .avatar{background:#da7756;color:#fff}
  .bubble{padding:12px 16px;border-radius:12px;font-size:14px;white-space:pre-wrap;word-break:break-word;max-width:100%}
  .msg.user .bubble{background:#1a3a5c;border:1px solid #1f6feb40;border-bottom-right-radius:4px}
  .msg.assistant .bubble{background:#1c1f26;border:1px solid #30363d;border-bottom-left-radius:4px}
  /* tool call / result cards */
  .tool-card{background:#161b22;border:1px solid #30363d;border-radius:8px;overflow:hidden;margin:4px 0;max-width:100%;font-size:13px}
  .tool-header{display:flex;align-items:center;gap:8px;padding:8px 12px;cursor:pointer;user-select:none}
  .tool-header:hover{background:#1c2128}
  .tool-icon{font-size:11px;color:#8b949e}
  .tool-name{font-weight:600;color:#d2a8ff;font-size:12px}
  .tool-label{font-size:11px;color:#8b949e;margin-left:auto}
  .tool-body{border-top:1px solid #30363d;padding:10px 12px;max-height:400px;overflow:auto}
  .tool-body pre{font-size:11px;color:#c9d1d9;white-space:pre-wrap;word-break:break-word}
  /* thinking */
  .thinking{background:#1c1f26;border:1px solid #d2992240;border-radius:8px;padding:10px 14px;font-size:13px;color:#d29922;font-style:italic;margin:4px 0;white-space:pre-wrap;word-break:break-word}
  .thinking-label{font-size:11px;color:#d29922;font-weight:600;margin-bottom:4px;font-style:normal}
  /* collapsible */
  details{margin:4px 0}
  details summary{cursor:pointer;user-select:none;list-style:none;display:flex;align-items:center;gap:6px}
  details summary::-webkit-details-marker{display:none}
  details summary::before{content:'▸';font-size:10px;color:#8b949e;transition:transform 0.15s}
  details[open] summary::before{transform:rotate(90deg)}
  /* system prompt */
  .sys-block{background:#161b22;border:1px solid #30363d;border-radius:8px;overflow:hidden}
  .sys-header{padding:10px 14px;font-size:12px;font-weight:600;color:#8b949e;display:flex;align-items:center;gap:6px;cursor:pointer;user-select:none}
  .sys-header:hover{background:#1c2128}
  .sys-body{border-top:1px solid #30363d;padding:12px 14px;max-height:500px;overflow:auto;font-size:12px;white-space:pre-wrap;word-break:break-word;color:#8b949e}
  /* tool defs */
  .tooldefs{font-size:12px;color:#8b949e}
  .tooldefs code{background:#1c2128;padding:1px 5px;border-radius:3px;font-size:11px;color:#d2a8ff}
  /* user-side context (environment_details etc) */
  .user-context{background:#11253d;border:1px solid #1f6feb30;border-radius:8px;overflow:hidden;max-width:90%;align-self:flex-end}
  .user-context .sys-header{color:#58a6ff}
  .user-context .sys-header:hover{background:#152d4a}
  /* response divider */
  .response-divider{display:flex;align-items:center;gap:10px;margin:16px 0 8px;font-size:12px;font-weight:600;color:#8b949e}
  .response-divider::before,.response-divider::after{content:'';flex:1;height:1px;background:#30363d}
  .response-divider span{white-space:nowrap;display:flex;align-items:center;gap:4px}
  .user-context .sys-body{color:#8b9fc2;border-top-color:#1f6feb20}
  .footer{text-align:center;padding:32px 0;font-size:12px;color:#484f58}
</style>
</head>
<body>
<div class="header">
  <h1>Request #${esc(String(entry.id))}</h1>
  <div class="meta-row">
    <span class="meta-tag plugin">${esc(entry.pluginId)}</span>
    ${entry.sessionKey ? `<span class="meta-tag" style="color:#79c0ff;border-color:#79c0ff30" title="${esc(entry.sessionKey)}">session: ${esc(entry.sessionKey)}</span>` : ''}
    <span class="meta-tag">${esc(entry.endpoint.toUpperCase())}</span>
    <span class="meta-tag model">${esc(entry.model)}</span>
    <span class="meta-tag ${entry.status === 'error' ? 'status-err' : 'status-ok'}">${esc(entry.status)} (${entry.statusCode})</span>
    ${entry.durationMs > 0 ? `<span class="meta-tag time">${entry.durationMs}ms</span>` : ''}
    <span class="meta-tag tokens">${fmtTokens(totalIn)} in / ${fmtTokens(entry.outputTokens)} out</span>
    ${(entry.cacheReadTokens || 0) > 0 ? `<span class="meta-tag" style="color:#79c0ff;border-color:#79c0ff30">cache: ${fmtTokens(entry.cacheReadTokens)}r / ${fmtTokens(entry.cacheWriteTokens || 0)}w</span>` : ''}
    <span class="meta-tag">${esc(time)}</span>
  </div>
</div>
<div class="content">
${errorHtml}
${requestParamsHtml}
${systemHtml}
${toolDefsHtml}
${messagesHtml}
</div>
<div class="footer">Exported from Open Claude Bridge</div>
</body>
</html>`

  const blob = new Blob([html], { type: 'text/html' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `request-${entry.id}.html`
  a.click()
  URL.revokeObjectURL(url)
}

// ---------------------------------------------------------------------------
// Message rendering helpers
// ---------------------------------------------------------------------------

function renderMessages(body: any, entry: RequestLogEntry): string {
  const messages: any[] = body?.messages
  if (!Array.isArray(messages) || messages.length === 0) {
    // Fallback: no structured messages, show raw responseText
    if (entry.responseText) {
      return `<div class="msg assistant"><div class="avatar">A</div><div class="bubble">${esc(entry.responseText)}</div></div>`
    }
    return ''
  }

  const parts: string[] = []

  for (const msg of messages) {
    if (msg.role === 'user') {
      parts.push(renderUserMessage(msg))
    } else if (msg.role === 'assistant') {
      parts.push(renderAssistantMessage(msg))
    }
  }

  // Append model response with visual separator
  if (entry.responseText) {
    const statusIcon = entry.status === 'success'
      ? '<span style="color:#3fb950">&#10003;</span>'
      : entry.status === 'error'
      ? '<span style="color:#f85149">&#10007;</span>'
      : '<span style="color:#d29922">&#8943;</span>'
    parts.push(`<div class="response-divider"><span>${statusIcon} Model Response</span></div>`)
    const trimmed = entry.responseText.trim()
    if (trimmed) {
      parts.push(`<div class="msg assistant"><div class="avatar">A</div><div class="bubble">${esc(trimmed)}</div></div>`)
    }
  }

  return parts.join('\n')
}

// System-injected block patterns (Claude Code hooks, memory, skills, suggestions)
const SYSTEM_BLOCK_PATTERNS: { re: RegExp; label: string }[] = [
  { re: /^\s*<system-reminder>\s*[\s\S]*?<\/system-reminder>\s*$/, label: 'System Reminder' },
  { re: /^\s*\[SUGGESTION MODE:[\s\S]*$/, label: 'Suggestion Mode' },
  { re: /^\s*<claude-mem-context>[\s\S]*?<\/claude-mem-context>\s*$/, label: 'Memory Context' },
  { re: /^\s*<claude_background_info>[\s\S]*?<\/claude_background_info>\s*$/, label: 'Background Info' },
  { re: /^\s*<fast_mode_info>[\s\S]*?<\/fast_mode_info>\s*$/, label: 'Fast Mode Info' },
  { re: /^\s*<env>[\s\S]*?<\/env>\s*$/, label: 'Environment' },
  { re: /^\s*<task-notification>[\s\S]*?<\/task-notification>\s*$/, label: 'Task Notification' },
]

// Regex to detect system-reminder that wraps a real user message
// e.g. <system-reminder>The user sent a new message while you were working:\n{text}\nIMPORTANT:...</system-reminder>
const USER_IN_SYSTEM_REMINDER_RE = /The user sent a new message while you were working:\s*\n([\s\S]*?)\n\s*IMPORTANT:/

/**
 * Classify a text block as system-injected. Returns label or null.
 * Special case: system-reminder that wraps a real user message → NOT classified as system
 * (returns null so the user message is shown).
 */
function classifySystemBlock(text: string): string | null {
  for (const p of SYSTEM_BLOCK_PATTERNS) {
    if (p.re.test(text)) {
      // system-reminder with user message inside → NOT system, show as user text
      if (p.label === 'System Reminder' && USER_IN_SYSTEM_REMINDER_RE.test(text)) {
        return null
      }
      return p.label
    }
  }
  return null
}

/**
 * Extract user message from a system-reminder wrapper if present.
 * Returns the user text or null if not a user-message wrapper.
 */
function extractUserFromSystemReminder(text: string): string | null {
  const m = USER_IN_SYSTEM_REMINDER_RE.exec(text)
  return m?.[1]?.trim() ?? null
}

function renderUserMessage(msg: any): string {
  const content = msg.content
  if (typeof content === 'string') {
    const trimmed = content.trim()
    return trimmed ? `<div class="msg user"><div class="avatar">U</div><div class="bubble">${esc(trimmed)}</div></div>` : ''
  }
  if (!Array.isArray(content)) return ''

  const parts: string[] = []
  // Collect consecutive system-injected blocks to render as a single collapsed group
  const systemBlocks: { label: string; text: string }[] = []

  const flushSystemBlocks = () => {
    if (systemBlocks.length === 0) return
    const count = systemBlocks.length
    const totalChars = systemBlocks.reduce((s, b) => s + b.text.length, 0)
    const labels = [...new Set(systemBlocks.map(b => b.label))].join(', ')
    const inner = systemBlocks.map(b =>
      `<div style="margin-bottom:8px;padding:6px 8px;background:rgba(139,148,158,0.08);border-radius:4px;"><div style="font-size:11px;color:#8b949e;margin-bottom:4px;font-weight:600;">${esc(b.label)}</div><pre style="margin:0;white-space:pre-wrap;font-size:12px;">${esc(b.text.length > 2000 ? b.text.slice(0, 2000) + '…' : b.text)}</pre></div>`
    ).join('\n')
    parts.push(`<details class="tool-card">
  <summary class="tool-header">
    <span class="tool-icon">⚙</span>
    <span class="tool-name">${esc(labels)}</span>
    <span class="tool-label">${count} block${count > 1 ? 's' : ''} · ${totalChars} chars</span>
  </summary>
  <div class="tool-body">${inner}</div>
</details>`)
    systemBlocks.length = 0
  }

  for (const block of content) {
    if (!block || typeof block !== 'object') continue
    if (block.type === 'text' && block.text) {
      const raw = block.text

      // Check if this is a system-injected block → collect for collapsed rendering
      const sysLabel = classifySystemBlock(raw)
      if (sysLabel) {
        systemBlocks.push({ label: sysLabel, text: raw })
        continue
      }

      // Not a system block — flush any pending system blocks first
      flushSystemBlocks()

      // System-reminder with embedded user message → extract user text + show system wrapper collapsed
      const embeddedUserText = extractUserFromSystemReminder(raw)
      if (embeddedUserText) {
        // Show the system wrapper collapsed
        systemBlocks.push({ label: 'System Reminder (user interrupt)', text: raw })
        flushSystemBlocks()
        // Show extracted user message as a normal user bubble
        parts.push(`<div class="msg user"><div class="avatar">U</div><div class="bubble">${esc(embeddedUserText)}</div></div>`)
        continue
      }

      // Extract <environment_details> as collapsible context block
      const envMatch = /<environment_details>\s*([\s\S]*?)\s*<\/environment_details>/.exec(raw)
      // Strip env details from main text
      let text = raw.replace(/<environment_details>[\s\S]*?<\/environment_details>/g, '').trim()
      // Extract from <user_message> wrapper if present
      const umMatch = /<user_message>\s*([\s\S]*?)\s*<\/user_message>/.exec(text)
      if (umMatch?.[1]) text = umMatch[1].trim()
      if (text) {
        parts.push(`<div class="msg user"><div class="avatar">U</div><div class="bubble">${esc(text)}</div></div>`)
      }
      if (envMatch?.[1]) {
        parts.push(renderContextBlock('🖥 Environment Details', envMatch[1].trim(), 'user'))
      }
    } else {
      // Non-text block — flush system blocks
      flushSystemBlocks()

      if (block.type === 'tool_result') {
        parts.push(renderToolResult(block))
      } else if (block.type === 'image') {
        parts.push(`<div class="msg user"><div class="avatar">U</div><div class="bubble"><em>[Image attachment]</em></div></div>`)
      }
    }
  }
  // Flush any trailing system blocks
  flushSystemBlocks()

  return parts.join('\n')
}

function renderAssistantMessage(msg: any): string {
  const content = msg.content
  if (typeof content === 'string') {
    const trimmed = content.trim()
    return trimmed ? `<div class="msg assistant"><div class="avatar">A</div><div class="bubble">${esc(trimmed)}</div></div>` : ''
  }
  if (!Array.isArray(content)) return ''

  const parts: string[] = []
  for (const block of content) {
    if (!block || typeof block !== 'object') continue
    if (block.type === 'thinking' && block.thinking) {
      const preview = block.thinking.length > 300 ? block.thinking.slice(0, 300) + '…' : block.thinking
      parts.push(`<details class="thinking"><summary class="thinking-label">💭 Thinking</summary>${esc(block.thinking)}</details>`)
    } else if (block.type === 'text' && block.text) {
      const trimmed = block.text.trim()
      if (trimmed) {
        parts.push(`<div class="msg assistant"><div class="avatar">A</div><div class="bubble">${esc(trimmed)}</div></div>`)
      }
    } else if (block.type === 'tool_use') {
      parts.push(renderToolUse(block))
    }
  }
  return parts.join('\n')
}

function renderToolUse(block: any): string {
  const name = block.name || 'unknown'
  const inputStr = block.input ? JSON.stringify(block.input, null, 2) : ''
  const id = block.id || ''
  return `<details class="tool-card">
  <summary class="tool-header">
    <span class="tool-icon">🔧</span>
    <span class="tool-name">${esc(name)}</span>
    <span class="tool-label">tool call${id ? ` · ${esc(id.slice(-8))}` : ''}</span>
  </summary>
  <div class="tool-body"><pre>${esc(inputStr)}</pre></div>
</details>`
}

function renderToolResult(block: any): string {
  const id = block.tool_use_id || ''
  let text = ''
  if (typeof block.content === 'string') {
    text = block.content
  } else if (Array.isArray(block.content)) {
    text = block.content
      .filter((b: any) => b?.type === 'text' && b.text)
      .map((b: any) => b.text)
      .join('\n')
  }

  const parts: string[] = []

  // Extract <user_message> if present → show as user bubble
  const userMsg = /<user_message>\s*([\s\S]*?)\s*<\/user_message>/.exec(text)
  if (userMsg?.[1]) {
    parts.push(`<div class="msg user"><div class="avatar">U</div><div class="bubble">${esc(userMsg[1].trim())}</div></div>`)
  }

  // Extract <environment_details> if present → show as collapsible
  const envMatch = /<environment_details>\s*([\s\S]*?)\s*<\/environment_details>/.exec(text)
  if (envMatch?.[1]) {
    parts.push(renderContextBlock('🖥 Environment Details', envMatch[1].trim(), 'user'))
  }

  // If user_message was found, also check for remaining content (strip known tags)
  if (userMsg) {
    const remaining = text
      .replace(/<user_message>[\s\S]*?<\/user_message>/g, '')
      .replace(/<environment_details>[\s\S]*?<\/environment_details>/g, '')
      .trim()
    if (remaining) {
      parts.push(`<details class="tool-card">
  <summary class="tool-header">
    <span class="tool-icon">📋</span>
    <span class="tool-name">additional context</span>
    <span class="tool-label">${remaining.length} chars</span>
  </summary>
  <div class="tool-body"><pre>${esc(remaining)}</pre></div>
</details>`)
    }
    return parts.join('\n')
  }

  // No user_message — show full tool result (strip env if already rendered)
  const displayText = envMatch ? text.replace(/<environment_details>[\s\S]*?<\/environment_details>/g, '').trim() : text
  if (displayText) {
    parts.push(`<details class="tool-card">
  <summary class="tool-header">
    <span class="tool-icon">📋</span>
    <span class="tool-name">tool result</span>
    <span class="tool-label">${id ? esc(id.slice(-8)) : ''} · ${text.length} chars</span>
  </summary>
  <div class="tool-body"><pre>${esc(displayText)}</pre></div>
</details>`)
  }
  return parts.join('\n')
}

function renderRequestParams(body: any): string {
  const params: string[] = []
  if (body.max_tokens != null) params.push(`max_tokens: ${body.max_tokens}`)
  if (body.temperature != null) params.push(`temperature: ${body.temperature}`)
  if (body.top_p != null) params.push(`top_p: ${body.top_p}`)
  if (body.top_k != null) params.push(`top_k: ${body.top_k}`)
  if (body.stream != null) params.push(`stream: ${body.stream}`)
  if (body.thinking) {
    const t = body.thinking
    params.push(`thinking: ${t.type || 'enabled'}${t.budget_tokens ? ` (budget: ${t.budget_tokens})` : ''}`)
  }
  if (body.tool_choice) {
    const tc = body.tool_choice
    params.push(`tool_choice: ${tc.type || JSON.stringify(tc)}${tc.disable_parallel_tool_use === false ? '' : tc.disable_parallel_tool_use ? ' (no parallel)' : ''}`)
  }
  if (params.length === 0) return ''
  return `<details class="sys-block">
  <summary class="sys-header">⚙ Request Parameters</summary>
  <div class="sys-body">${params.map(p => esc(p)).join('<br>')}</div>
</details>`
}

function renderSystem(body: any): string {
  if (!body?.system) return ''
  let text = ''
  if (typeof body.system === 'string') {
    text = body.system
  } else if (Array.isArray(body.system)) {
    text = body.system
      .filter((b: any) => b?.type === 'text' && b.text)
      .map((b: any) => b.text)
      .join('\n\n')
  }
  if (!text.trim()) return ''
  const preview = text.length > 500 ? text.slice(0, 500) + '…' : text
  return `<details class="sys-block">
  <summary class="sys-header">📝 System Prompt (${text.length} chars)</summary>
  <div class="sys-body">${esc(text)}</div>
</details>`
}

function renderToolDefs(body: any): string {
  const tools: any[] = body?.tools
  if (!Array.isArray(tools) || tools.length === 0) return ''
  const names = tools.map((t: any) => t.name).filter(Boolean)
  return `<details class="sys-block">
  <summary class="sys-header">🔧 Available Tools (${names.length})</summary>
  <div class="sys-body tooldefs">${names.map(n => `<code>${esc(n)}</code>`).join(' ')}</div>
</details>`
}

function renderContextBlock(title: string, content: string, side: 'system' | 'user' = 'system'): string {
  const cls = side === 'user' ? 'sys-block user-context' : 'sys-block'
  return `<details class="${cls}">
  <summary class="sys-header">📋 ${esc(title)} (${content.length} chars)</summary>
  <div class="sys-body">${esc(content)}</div>
</details>`
}

function esc(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')
}

