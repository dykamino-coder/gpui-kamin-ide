// Только числовой usage: ни prompts, ни tool inputs в ledger не записываются.
import type { SynthAssistantEntry } from '../proxy/entry-synthesizer'
import { getDb } from './database/lifecycle'

type UsageEntry = Pick<SynthAssistantEntry, 'timestamp' | 'isSidechain'> & {
  message: Pick<SynthAssistantEntry['message'], 'id' | 'model' | 'usage'>
  usage?: SynthAssistantEntry['usage']
}

/** Keep only accounting metadata until the first JSONL file reveals its id. */
export function createProxyUsageRecorder(
  owner: { tokenId: string; userName: string },
  onError: (error: unknown) => void,
  persist = recordProxyUsage,
) {
  let sessionId: string | undefined
  const pending = new Map<string, UsageEntry>()
  const write = (entry: UsageEntry) => {
    void persist(entry, { ...owner, sessionId: sessionId! }).catch(onError)
  }
  return {
    record(entry: SynthAssistantEntry) {
      const usage = entry.usage ?? entry.message.usage
      if (!usage) return
      const numeric: UsageEntry = {
        timestamp: entry.timestamp,
        isSidechain: entry.isSidechain,
        message: {
          id: entry.message.id,
          model: entry.message.model,
          usage: {
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            cache_read_input_tokens: usage.cache_read_input_tokens,
            cache_creation_input_tokens: usage.cache_creation_input_tokens,
            cache_creation: usage.cache_creation ? { ...usage.cache_creation } : undefined,
          },
        },
      }
      if (sessionId) write(numeric)
      else pending.set(entry.message.id, numeric)
    },
    bind(id: string) {
      sessionId = id
      for (const entry of pending.values()) write(entry)
      pending.clear()
    },
  }
}

export async function recordProxyUsage(
  entry: UsageEntry,
  owner: { sessionId: string; tokenId: string; userName: string },
): Promise<void> {
  const usage = entry.usage ?? entry.message.usage
  if (!entry.message.id || !usage) return
  const count = (n: number | undefined): number => (Number.isSafeInteger(n) && n! > 0 ? n! : 0)
  const written = count(usage.cache_creation_input_tokens)
  const db = await getDb()
  await db.run(
    `INSERT INTO proxy_usage
    (message_id, session_id, timestamp, model, input_tokens, output_tokens,
      cache_read_tokens, cache_write_tokens, cache_write_1h_tokens, token_id, user_name, is_subagent)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    ON CONFLICT (message_id) DO UPDATE SET
      input_tokens = GREATEST(proxy_usage.input_tokens, excluded.input_tokens),
      output_tokens = GREATEST(proxy_usage.output_tokens, excluded.output_tokens),
      cache_read_tokens = GREATEST(proxy_usage.cache_read_tokens, excluded.cache_read_tokens),
      cache_write_tokens = GREATEST(proxy_usage.cache_write_tokens, excluded.cache_write_tokens),
      cache_write_1h_tokens = GREATEST(proxy_usage.cache_write_1h_tokens, excluded.cache_write_1h_tokens)`,
    [
      entry.message.id,
      owner.sessionId,
      entry.timestamp,
      entry.message.model,
      count(usage.input_tokens),
      count(usage.output_tokens),
      count(usage.cache_read_input_tokens),
      written,
      Math.min(written, count(usage.cache_creation?.ephemeral_1h_input_tokens)),
      owner.tokenId,
      owner.userName,
      entry.isSidechain ? 1 : 0,
    ],
  )
}
