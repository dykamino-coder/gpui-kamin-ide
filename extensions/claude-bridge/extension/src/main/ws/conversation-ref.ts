import { createHash } from 'node:crypto'

/** Stable pseudonym of a CLI conversation id (INC-2026-0070). Identical to the
 *  Bridge server's `conversationRef` (server/src/core/logging/lifecycle.ts) so a
 *  tab's identity in the client incident log can be matched with the server's
 *  lifecycle journal across container recreations without either log holding
 *  the raw id. 48 bits of a domain-separated digest do not reveal a random
 *  UUID; only a holder of the raw id can confirm a match. */
export function conversationRef(id: string): string {
  return createHash('sha256').update(`kamin-conversation-ref-v1:${id}`).digest('hex').slice(0, 12)
}
