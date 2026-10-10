// Synthetic public formats: agent-replay.test.ts, jsonl-watcher-live-compact.test.ts,
// session-cost.test.ts and RUNTIME_RELIABILITY.md BR-20. No recorded private data.
export function projectSlug(resolved) {
  const slug = resolved.replace(/[^a-zA-Z0-9]/g, '-')
  if (slug.length <= 200) return slug
  let hash = 0
  for (let i = 0; i < resolved.length; i++) hash = ((hash << 5) - hash + resolved.charCodeAt(i)) | 0
  return `${slug.slice(0, 200)}-${Math.abs(hash).toString(36)}`
}
export function records(sessionId, start = 0) {
  let seq = start
  const entry = (type, message, extra = {}) => {
    seq++
    return {
      type,
      uuid: `${sessionId}-${seq}`,
      sessionId,
      timestamp: new Date(Date.UTC(2026, 0, 1) + seq * 1000).toISOString(),
      ...(message ? { message } : {}),
      ...extra,
    }
  }
  const user = (content, extra) => entry('user', { role: 'user', content }, extra)
  const assistant = (content, tokens = 16000, extra) =>
    entry(
      'assistant',
      {
        id: `msg-${sessionId}-${seq + 1}`,
        role: 'assistant',
        model: 'claude-opus-5-5',
        content,
        usage: { input_tokens: tokens, output_tokens: 20, cache_read_input_tokens: 0, cache_creation_input_tokens: 0 },
      },
      extra,
    )
  const tool = (name, input) => assistant([{ type: 'tool_use', id: `toolu-${sessionId}-${seq + 1}`, name, input }])
  const result = (id, text, error = false) =>
    user([
      { type: 'tool_result', tool_use_id: id, content: [{ type: 'text', text }], ...(error ? { is_error: true } : {}) },
    ])
  const idle = (name, emitted) =>
    user(
      `Another Claude session sent a message:\n<teammate-message teammate_id="${name}" color="blue">\n${JSON.stringify({ type: 'idle_notification', from: name, timestamp: emitted ?? new Date(Date.UTC(2026, 0, 1) + (seq + 1) * 1000).toISOString(), idleReason: 'available' })}\n</teammate-message>`,
    )
  const report = (name, text) => user(`<teammate-message teammate_id="${name}" color="blue">${text}</teammate-message>`)
  const compact = (postTokens = 15098) =>
    entry('system', undefined, {
      subtype: 'compact_boundary',
      compactMetadata: { trigger: 'manual', preTokens: 978010, ...(postTokens === null ? {} : { postTokens }) },
    })
  return { user, assistant, tool, result, idle, report, compact }
}
export const usageLayouts = {
  legacy:
    'Current session\n23% used\nResets 5pm (UTC)\nCurrent week (all models)\n42% used\nResets Oct 15 5pm (UTC)\nCurrent week (Sonnet only)\n7% used\nResets Oct 15 5pm (UTC)\nExtra usage not enabled\nEsc to cancel\n',
  fable:
    'Current session\n23% used\nResets 5pm (UTC)\nCurrent week (all models)\n42% used\nResets Oct 15 5pm (UTC)\nLimited time promotion: extra capacity.\nCurrent week (Fable)\n7% used\nResets Oct 15 5pm (UTC)\nExtra usage not enabled\nEsc to cancel\n',
  error: 'Error: Failed to load usage data\n',
  unknown: 'Current fortnight (experimental)\n11% used\nResets Nov 1 (UTC)\nEsc to cancel\n',
}
