// Users list: the ChatGPT (codex) part of a user's totals. codex runs on the
// server's ChatGPT plan, so its price is shown only as the API equivalent at
// OpenAI list prices, and tokens of a model with no published price stay
// tokens — never a guessed or Claude rate.

import type { UserSummary } from '../../services/api-client'
import { formatCost, formatTokens } from '../../utils/formatters'

export function codexCostLine(u: UserSummary): { text: string; tooltip: string } | null {
  const c = u.codex
  if (!c || (c.apiEquivalentUsd <= 0 && c.unpricedTokens <= 0)) return null
  const parts: string[] = []
  const notes: string[] = []
  if (c.apiEquivalentUsd > 0) {
    parts.push(`≈${formatCost(c.apiEquivalentUsd)} API`)
    notes.push(
      `ChatGPT: billed by the ChatGPT plan. ≈${formatCost(c.apiEquivalentUsd)} is what the same tokens cost on the OpenAI API (list prices as of ${c.pricesAsOf}).`,
    )
  }
  if (c.unpricedTokens > 0) {
    parts.push(`${formatTokens(c.unpricedTokens)} tok`)
    notes.push(`No published API price for ${c.unpricedModels.join(', ')}: shown as tokens.`)
  }
  return { text: parts.join(' + '), tooltip: notes.join(' ') }
}

export function engineTokensTooltip(u: UserSummary, total: number): string {
  const lines = [`Total tokens: ${total.toLocaleString()}`]
  for (const [engine, label] of [
    ['claude', 'Claude'],
    ['codex', 'ChatGPT'],
  ] as const) {
    const e = u.engines?.[engine]
    if (!e) continue
    const t = e.tokens.input + e.tokens.output + e.tokens.cacheRead + e.tokens.cacheCreation
    lines.push(`${label}: ${t.toLocaleString()} tokens, ${e.responses} responses, ${e.sessions} sessions`)
  }
  return lines.join('\n')
}
