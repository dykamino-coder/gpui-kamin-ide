import { stat } from 'node:fs/promises'
import type { CodexTranscriptEntry } from './transcript'
import { CodexConsoleRenderer } from './console-render'
import { readJsonlLines } from '../pty/jsonl-lines'

/** Recent visible messages for a new terminal process. The paged transcript in Chat
 * holds the complete history. Never render raw tool output or opaque thinking blocks. */
export async function recentConsoleMessages(file: string, threadId: string): Promise<string> {
  const renderer = new CodexConsoleRenderer()
  const messages: string[] = []
  try {
    const { size } = await stat(file)
    const start = Math.max(0, size - 128 * 1024)
    for await (const line of readJsonlLines(file, { start: Math.max(0, start - 1), end: size })) {
      if (line.start < start) continue
      let entry: CodexTranscriptEntry
      try {
        entry = JSON.parse(line.text)
      } catch {
        continue
      }
      if (entry?.engine !== 'codex' || entry.sessionId !== threadId || entry.isSidechain || entry.isMeta) continue
      if (entry.type !== 'user' && entry.type !== 'assistant') continue
      if (entry.message?.role !== entry.type) continue
      const content = entry.message.content
      const text =
        typeof content === 'string'
          ? content
          : Array.isArray(content)
            ? content
                .flatMap((block) => (block?.type === 'text' && typeof block.text === 'string' ? [block.text] : []))
                .join('\n')
            : ''
      const at = Date.parse(entry.timestamp ?? '')
      if (!text || !Number.isFinite(at)) continue
      messages.push(
        renderer.render({
          kind: entry.type === 'user' ? 'message.user' : 'message.assistant',
          id: entry.uuid,
          turnId: '',
          threadId,
          at,
          text,
        }),
      )
    }
  } catch {
    // A missing or rotated transcript does not prevent the native thread from resuming.
  }
  return messages.length ? 'Recent messages from this chat\r\n' + messages.join('') + '\r\nLive events\r\n' : ''
}
