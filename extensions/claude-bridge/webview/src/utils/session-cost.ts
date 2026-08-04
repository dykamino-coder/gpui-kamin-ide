// Cost + context estimation. Constants mirror CLI `modelCost.ts` pricing
// (tiers COST_TIER_3_15, COST_TIER_15_75, COST_TIER_5_25). If Anthropic
// adjusts pricing, update these along with CLI's configs.js.
//
// Per-million-token rates in USD.
export interface CostTier {
  input: number
  output: number
  cacheWrite: number
  cacheRead: number
}

const TIER_3_15: CostTier = { input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 }
const TIER_15_75: CostTier = { input: 15, output: 75, cacheWrite: 18.75, cacheRead: 1.5 }
const TIER_5_25: CostTier = { input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 }
const TIER_HAIKU: CostTier = { input: 1, output: 5, cacheWrite: 1.25, cacheRead: 0.1 }

/** Infer pricing tier from a model identifier. Falls back to Sonnet rate
 *  when nothing matches — keeps the per-session cost estimate within an
 *  order of magnitude for unknown models rather than silently showing $0. */
export function tierForModel(modelId: string | undefined | null): CostTier {
  const m = (modelId || '').toLowerCase()
  if (m.includes('haiku')) return TIER_HAIKU
  // Opus 5 — текущий дефолт (тот же тариф, что и линейка 4.5+).
  if (m.includes('opus-5')) return TIER_5_25
  if (m.includes('opus-4-1') || m.includes('opus-4-0') || m.includes('opus-4')) {
    // Ветка 4.x ЖИВА только для стоимости СТАРЫХ логов (4.8 и ранее из
    // пикера выпилены): Opus 4.5+ — дешёвый tier, 4.0/4.1 — дорогой.
    if (m.includes('opus-4-5') || m.includes('opus-4-6') || m.includes('opus-4-7') || m.includes('opus-4-8')) return TIER_5_25
    return TIER_15_75
  }
  return TIER_3_15
}

/** Context window size in tokens. CLI reads this from model configs; we
 *  approximate by model-name suffix — the `[1m]` tag marks the extended
 *  1M-context variant, everything else defaults to the standard 200K. */
export function contextLimitForModel(modelId: string | undefined | null): number {
  const m = (modelId || '').toLowerCase()
  // Легаси-тег `[1m]` (эпоха Opus 4.x): старые сессии могли писать
  // `claude-opus-4-8[1m]` — уважаем при подсчёте их окна. Новых [1m]-id
  // пикер не выдаёт: у Opus 5 миллион нативный.
  if (m.includes('[1m]') || m.includes('1m')) return 1_000_000
  // Models whose window is 1M NATIVELY. There is no `[1m]` suffix for these
  // because 1M is the base, not a beta — Opus 5, Fable 5, Mythos 5 and
  // Sonnet 5 all ship at 1M by default. Without this branch they fell through
  // to 200K, so the usage bar read "full" at a fifth of the real window (the
  // Fable indicator the user flagged). Opus 4.x (200K base) and Haiku 4.5
  // (genuinely 200K) are intentionally NOT matched here.
  if (/claude-(?:fable|mythos|opus)-5|claude-sonnet-5/.test(m)) return 1_000_000
  return 200_000
}

export interface UsageBlock {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
}

export function costForUsage(usage: UsageBlock, tier: CostTier): number {
  const inTok = usage.input_tokens ?? 0
  const outTok = usage.output_tokens ?? 0
  const cacheRead = usage.cache_read_input_tokens ?? 0
  const cacheWrite = usage.cache_creation_input_tokens ?? 0
  return (
    (inTok * tier.input +
      outTok * tier.output +
      cacheWrite * tier.cacheWrite +
      cacheRead * tier.cacheRead) / 1_000_000
  )
}

/** Effective input token count for context-usage display: regular input +
 *  cache read + cache creation. Matches how the API counts window pressure
 *  (cached tokens still occupy the context, they're just cheap). */
export function effectiveInputTokens(usage: UsageBlock): number {
  return (usage.input_tokens ?? 0) +
    (usage.cache_read_input_tokens ?? 0) +
    (usage.cache_creation_input_tokens ?? 0)
}

export function formatCost(usd: number): string {
  if (usd < 0.01) return `$${usd.toFixed(4)}`
  if (usd < 1) return `$${usd.toFixed(3)}`
  return `$${usd.toFixed(2)}`
}

/** A minimal assistant JSONL row shape for stats. */
interface StatEntry {
  type?: string
  model?: unknown
  usage?: UsageBlock
  message?: { id?: string; model?: unknown; usage?: UsageBlock }
  /** Субагентский ход: у него своё окно контекста, к главному бару не относится. */
  isSidechain?: boolean
  subagentId?: string
}

export interface ContextStats {
  /** Context-window occupancy as a rounded percentage of the model limit. */
  pct: number
  /** Effective input tokens driving the bar (window occupancy). */
  used: number
  /** Model context-window size the pct is against. */
  limit: number
  /** Cumulative $ cost across the segment (deduped by message.id). */
  cost: number
  /** Model id used to resolve the limit. */
  model: string
}

/** Cost + context-pressure for ONE compact segment's entries (already sliced).
 *
 *  `isLiveSegment` decides WHICH turn drives the bar — this is the whole fix for
 *  the "context doesn't reset after /compact and then freezes" report:
 *   • live "Current" segment → the LATEST real turn (the current window occupancy,
 *     matching the CLI's `/context`). A `/compact` collapses the window, so every
 *     post-compact turn is smaller than the pre-compact peak; a peak-based bar
 *     would stick at that peak forever.
 *   • archived segment → the PEAK turn ("how full did it get before it was
 *     compacted"), which is the meaningful number for a finished sub-conversation.
 *
 *  `<synthetic>` rows (compaction markers, error stubs) are skipped so they can't
 *  collapse the bar to 0%. Cost dedups by message.id (one model turn is emitted as
 *  N JSONL rows sharing a message.id when it has N parallel tool_use blocks). */
export function computeContextStats(
  entries: StatEntry[],
  isLiveSegment: boolean,
  tabModel: string | null | undefined,
): ContextStats | null {
  let cumulativeCost = 0
  let peakUsage: UsageBlock | null = null
  let peakUsedTokens = -1
  let peakModel: string | null = null
  let latestUsage: UsageBlock | null = null
  let latestUsedTokens = -1
  let latestModel: string | null = null
  const seenMessages = new Set<string>()

  for (const e of entries) {
    if (e.type !== 'assistant') continue
    // Субагентские ходы имеют СВОЁ окно контекста — включённые в общий стор
    // (canonical sidechain-строки + живые streaming-стабы) они дёргали главный
    // бар: он снапался к окну субагента и прыгал обратно (жалоба юзера).
    if (e.isSidechain || e.subagentId) continue
    const model = e.model ?? e.message?.model
    if (model === '<synthetic>') continue
    const usage = e.usage ?? e.message?.usage
    if (!usage) continue
    const messageId = e.message?.id
    const shouldCount = !messageId || !seenMessages.has(messageId)
    if (messageId) seenMessages.add(messageId)
    if (shouldCount) cumulativeCost += costForUsage(usage, tierForModel(typeof model === 'string' ? model : null))
    const used = effectiveInputTokens(usage)
    if (used > peakUsedTokens) {
      peakUsedTokens = used
      peakUsage = usage
      if (typeof model === 'string') peakModel = model
    }
    // Chronological input → last assignment is the newest real turn.
    latestUsedTokens = used
    latestUsage = usage
    if (typeof model === 'string') latestModel = model
  }

  if (!peakUsage) return null
  const useUsage = isLiveSegment ? (latestUsage ?? peakUsage) : peakUsage
  const useUsedTokens = isLiveSegment && latestUsedTokens >= 0 ? latestUsedTokens : peakUsedTokens
  const useModel = isLiveSegment ? (latestModel ?? peakModel) : peakModel
  // Окно контекста: у сессии ДВА источника модели, и легаси может застрять в
  // ЛЮБОМ из них (таб хранил opus-4-8 при витках на 1M — старый инцидент; витки
  // хранят opus-4-8 до апгрейда, а таб уже живой opus-5 после hot-switch —
  // инцидент «100% при 510K»). Берём максимум окон обоих — окно определяется
  // тем, на чём сессия реально может работать сейчас.
  const tabLimit = tabModel ? contextLimitForModel(tabModel) : 0
  const jsonlLimit = useModel ? contextLimitForModel(useModel) : 0
  const limit = Math.max(tabLimit, jsonlLimit) || contextLimitForModel('claude-opus-5')
  const modelForLimit = (tabLimit >= jsonlLimit ? tabModel : useModel) ?? useModel ?? tabModel ?? 'claude-opus-5'
  const used = useUsedTokens >= 0 ? useUsedTokens : effectiveInputTokens(useUsage)
  const pct = Math.min(100, Math.round((used / limit) * 100))
  return { pct, used, limit, cost: cumulativeCost, model: modelForLimit }
}
