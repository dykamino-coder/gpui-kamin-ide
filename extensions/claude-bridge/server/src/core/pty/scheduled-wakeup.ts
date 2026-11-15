// A chat that scheduled its own wakeup (the CLI's ScheduleWakeup tool) has
// work due later even though it sits idle at its prompt now. Hibernation must
// not stop such a CLI: its timer lives inside the process. The transcript is
// where the server sees the call, so every live batch of entries passes here.

import type { PtySession } from './types'

/** Longest delay the CLI accepts; anything larger is clamped by it, too. */
const MAX_DELAY_S = 3600

interface ToolUseBlock {
  type?: unknown
  name?: unknown
  input?: { delaySeconds?: unknown }
}

/** Record the latest due time of a ScheduleWakeup found in `entries`. Entries
 *  replayed from history are skipped by the caller: their timers died with
 *  the process that set them. */
export function noteScheduledWakeups(
  session: Pick<PtySession, 'scheduledWakeupAt'>,
  entries: unknown[],
  now = Date.now(),
): void {
  for (const entry of entries) {
    const message = (entry as { type?: unknown; message?: { content?: unknown } } | null)?.message
    if ((entry as { type?: unknown })?.type !== 'assistant' || !Array.isArray(message?.content)) continue
    for (const block of message.content as ToolUseBlock[]) {
      if (block?.type !== 'tool_use' || block.name !== 'ScheduleWakeup') continue
      const delay = Number(block.input?.delaySeconds)
      const seconds = Number.isFinite(delay) && delay > 0 ? Math.min(delay, MAX_DELAY_S) : MAX_DELAY_S
      const due = now + seconds * 1000
      if (!session.scheduledWakeupAt || due > session.scheduledWakeupAt) session.scheduledWakeupAt = due
    }
  }
}
