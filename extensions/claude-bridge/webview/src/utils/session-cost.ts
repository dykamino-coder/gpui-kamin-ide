// Cost + context estimation using Anthropic's published base API rates.
// Keep this estimate aligned with server dashboard pricing when models change.
// ChatGPT (codex) lines use the OpenAI list prices shared with the dashboard.
//
// Per-million-token rates in USD.
import { isEngineUsageLine } from '../../../server/src/shared/engine-accounting'
import { OPENAI_PRICES_AS_OF, openAiRequestCost } from '../../../server/src/shared/openai-pricing'

export interface CostTier {
  input: number
  output: number
  cacheWrite: number
  cacheRead: number
  /** Long-prompt rate: applies to the WHOLE request once its prompt (input +
   *  cache read + cache write) exceeds `tokens` (Haiku 5.5: 100K). */
  above?: { tokens: number; tier: CostTier }
  /** Fast-mode multiplier for every component when the API reports
   *  `usage.speed === 'fast'` (CLI 2.1.322 bills Fable 5.2 and Opus 5.5 fast
   *  turns at exactly 2× the standard rates). */
  fast?: number
}

const TIER_3_15: CostTier = { input: 3, output: 15, cacheWrite: 3.75, cacheRead: 0.3 }
const TIER_2_10: CostTier = { input: 2, output: 10, cacheWrite: 2.5, cacheRead: 0.2 }
const TIER_15_75: CostTier = { input: 15, output: 75, cacheWrite: 18.75, cacheRead: 1.5 }
const TIER_5_25: CostTier = { input: 5, output: 25, cacheWrite: 6.25, cacheRead: 0.5 }
const TIER_4_20: CostTier = { input: 4, output: 20, cacheWrite: 5, cacheRead: 0.2, fast: 2 }
const TIER_FABLE_5_1: CostTier = { input: 10, output: 50, cacheWrite: 12.5, cacheRead: 0.25 }
const TIER_FABLE_5_2: CostTier = { input: 8, output: 40, cacheWrite: 10, cacheRead: 0.2, fast: 2 }
const TIER_HAIKU: CostTier = { input: 1, output: 5, cacheWrite: 1.25, cacheRead: 0.1 }
// Haiku 5.5 (platform.claude.com pricing): $0.10/$0.50 up to a 100K-token
// prompt, $0.50/$2.50 above; cache reads $0.01/$0.05. Cache writes follow the
// usual 1.25× input rate.
const TIER_HAIKU_5_5: CostTier = {
  input: 0.1, output: 0.5, cacheWrite: 0.125, cacheRead: 0.01,
  above: { tokens: 100_000, tier: { input: 0.5, output: 2.5, cacheWrite: 0.625, cacheRead: 0.05 } },
}

/** Infer pricing tier from a model identifier. Falls back to Sonnet rate
 *  when nothing matches — keeps the per-session cost estimate within an
 *  order of magnitude for unknown models rather than silently showing $0. */
export function tierForModel(modelId: string | undefined | null): CostTier {
  const m = (modelId || '').toLowerCase()
  if (m.includes('haiku-5')) return TIER_HAIKU_5_5
  if (m.includes('haiku')) return TIER_HAIKU
  if (m.includes('fable-5-2')) return TIER_FABLE_5_2
  if (m.includes('fable-5-1')) return TIER_FABLE_5_1
  if (m.includes('opus-5-5')) return TIER_4_20
  // Opus 5 and legacy 4.5+ use the previous rate.
  if (m.includes('opus-5')) return TIER_5_25
  if (m.includes('opus-4-1') || m.includes('opus-4-0') || m.includes('opus-4')) {
    // Ветка 4.x ЖИВА только для стоимости СТАРЫХ логов (4.8 и ранее из
    // пикера выпилены): Opus 4.5+ — дешёвый tier, 4.0/4.1 — дорогой.
    if (m.includes('opus-4-5') || m.includes('opus-4-6') || m.includes('opus-4-7') || m.includes('opus-4-8')) return TIER_5_25
    return TIER_15_75
  }
  if (m.includes('sonnet-5-5')) return { ...TIER_2_10, cacheRead: 0.1 }
  if (m.includes('sonnet-5')) return TIER_2_10
  return TIER_3_15
}

/** Fallback context window size in tokens, used only while the CLI has not
 *  reported its own `context_window.context_window_size` (see
 *  computeContextStats). Every model the current CLI offers — Opus/Sonnet/
 *  Haiku 5.5, Fable 5.x — has a 1M window, and so do the aliases the CLI keeps
 *  in a session (`opus`, `fable`, `sonnet`, `haiku`, `default`, …: 2.1.321
 *  migrates saved Fable picks to the bare `fable` alias). Only ids that NAME a
 *  pre-5 family are 200K: those windows really were 200K unless the session
 *  ran the `[1m]` variant. Keying 1M on a whitelist of 5.x spellings instead
 *  made every alias or unfamiliar id read "100%" at 200K tokens. */
export function contextLimitForModel(modelId: string | undefined | null): number {
  const m = (modelId || '').toLowerCase()
  // `claude-opus-4-8[1m]` и подобные легаси-id расширенного окна.
  if (m.includes('1m')) return 1_000_000
  // claude-3-5-sonnet-…, claude-opus-4-8, claude-sonnet-4-6, claude-haiku-4-5,
  // Bedrock/Vertex/gateway spellings of the same families.
  if (/claude-(?:instant|[23])\b|(?:opus|sonnet|haiku)-[34]\b/.test(m)) return 200_000
  return 1_000_000
}

export interface UsageBlock {
  input_tokens?: number
  output_tokens?: number
  cache_read_input_tokens?: number
  cache_creation_input_tokens?: number
  cache_creation?: { ephemeral_5m_input_tokens?: number; ephemeral_1h_input_tokens?: number }
  /** `'fast'` on fast-mode turns; absent or `null` at standard speed. */
  speed?: string | null
}

export function costForUsage(usage: UsageBlock, base: CostTier): number {
  const inTok = usage.input_tokens ?? 0
  const outTok = usage.output_tokens ?? 0
  const cacheRead = usage.cache_read_input_tokens ?? 0
  const cacheWrite = usage.cache_creation_input_tokens ?? 0
  const tier = base.above && inTok + cacheRead + cacheWrite > base.above.tokens ? base.above.tier : base
  const hourWrite = Math.max(0, Math.min(cacheWrite, usage.cache_creation?.ephemeral_1h_input_tokens ?? 0))
  const speed = usage.speed === 'fast' ? (base.fast ?? 1) : 1
  return speed * (
    (inTok * tier.input +
      outTok * tier.output +
      cacheWrite * tier.cacheWrite + hourWrite * (2 * tier.input - tier.cacheWrite) +
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
  subtype?: string
  compactMetadata?: { postTokens?: number }
  model?: unknown
  usage?: UsageBlock
  message?: { id?: string; model?: unknown; usage?: UsageBlock }
  /** Субагентский ход: у него своё окно контекста, к главному бару не относится. */
  isSidechain?: boolean
  subagentId?: string
  /** `codex` on ChatGPT lines; absent on Claude CLI lines. */
  engine?: string
  uuid?: string
  /** Model window codex reported with a usage line. */
  contextWindow?: number
}

export interface ContextStats {
  /** Context-window occupancy as a rounded percentage of the model limit. */
  pct: number | null
  /** Effective input tokens driving the bar (window occupancy). */
  used: number | null
  /** Token count the pct is against: the auto-compact point when known. */
  limit: number
  /** `compact` — where the CLI auto-compacts (server policy); `cli` — the
   *  CLI-reported model window (older servers); `model` — estimated from the id. */
  limitSource: 'compact' | 'cli' | 'engine' | 'model'
  /** Cumulative $ cost of the segment's Claude responses (deduped by message.id). */
  cost: number
  /** ChatGPT (codex) responses: billed by the ChatGPT plan, so only the
   *  API-equivalent list price of priced models; null when the segment has
   *  no codex usage. */
  codexCost: number | null
  /** codex tokens of models without a published API price (shown as tokens). */
  codexUnpricedTokens: number
  /** Model id used to estimate the limit (empty when none is known). */
  model: string
  /** Compact metadata is provisional until a real post-boundary usage arrives. */
  freshness: 'usage' | 'compact' | 'pending'
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
  /** The session's `session:usage`: `autoCompactAt` is the meter's 100%;
   *  `contextWindowSize` (older servers) is the live model window. */
  session?: { autoCompactAt?: number | null; contextWindowSize?: number | null } | null,
): ContextStats | null {
  let cumulativeCost = 0
  let codexCost: number | null = null
  let codexUnpricedTokens = 0
  let peakUsage: UsageBlock | null = null
  let peakUsedTokens = -1
  let peakModel: string | null = null
  let peakWindow = 0
  let latestUsage: UsageBlock | null = null
  let latestUsedTokens = -1
  let latestModel: string | null = null
  let latestWindow = 0
  const messageCosts = new Map<string, number>()
  let lastCompact: { postTokens?: number } | null = null

  for (const e of entries) {
    if ((e.isSidechain || e.subagentId) && e.type !== 'assistant') continue
    if (isLiveSegment && e.type === 'system' && e.subtype === 'compact_boundary') {
      // A completed compact invalidates previous Current occupancy immediately.
      // Synthetic summaries and hook errors are not newer model usage.
      lastCompact = e.compactMetadata ?? {}
      latestUsage = null
      latestUsedTokens = -1
      continue
    }
    // ChatGPT (codex): one `api_usage` line per model response; its item lines
    // carry no usage (shared/engine-accounting.ts). The plan bills it, so the
    // price is the API equivalent at OpenAI list prices, never a Claude rate.
    const codex = isEngineUsageLine(e)
    if (!codex && e.type !== 'assistant') continue
    const model = e.model ?? e.message?.model
    if (model === '<synthetic>') continue
    const usage = e.usage ?? e.message?.usage
    if (!usage) continue
    const modelId = typeof model === 'string' ? model : null
    if (codex) {
      const tokens = {
        input: usage.input_tokens ?? 0,
        cacheRead: usage.cache_read_input_tokens ?? 0,
        cacheWrite: usage.cache_creation_input_tokens ?? 0,
        output: usage.output_tokens ?? 0,
      }
      const usd = openAiRequestCost(modelId, tokens)
      codexCost = (codexCost ?? 0) + (usd ?? 0)
      if (usd === null) codexUnpricedTokens += tokens.input + tokens.cacheRead + tokens.cacheWrite + tokens.output
    } else {
      const messageId = e.message?.id
      const cost = costForUsage(usage, tierForModel(modelId))
      if (messageId) {
        const previous = messageCosts.get(messageId) ?? 0
        cumulativeCost += Math.max(previous, cost) - previous
        messageCosts.set(messageId, Math.max(previous, cost))
      } else cumulativeCost += cost
    }
    // Стоимость включает субагентов, их отдельное окно не меняет главный бар.
    if (e.isSidechain || e.subagentId) continue
    const window = codex && typeof e.contextWindow === 'number' ? e.contextWindow : 0
    const used = effectiveInputTokens(usage)
    if (used > peakUsedTokens) {
      peakUsedTokens = used
      peakUsage = usage
      peakWindow = window
      if (modelId) peakModel = modelId
    }
    // Chronological input → last assignment is the newest real turn.
    latestUsedTokens = used
    latestUsage = usage
    latestWindow = window
    if (modelId) latestModel = modelId
  }

  if (!peakUsage && !lastCompact) return null
  const useUsage = isLiveSegment ? (latestUsage ?? peakUsage) : peakUsage
  const useUsedTokens = isLiveSegment && latestUsedTokens >= 0 ? latestUsedTokens : peakUsedTokens
  const useModel = isLiveSegment ? (latestModel ?? peakModel) : peakModel
  // 100% — точка автосжатия, которую сервер задаёт CLI и сообщает в
  // session:usage: сжатие к ней и приводит, в любом сегменте. Без неё (старый
  // сервер) — окно модели из status line, только для текущего сегмента:
  // архивный мог идти на другой модели.
  const valid = (n: number | null | undefined): number => (typeof n === 'number' && Number.isSafeInteger(n) && n > 0 ? n : 0)
  const compactLimit = valid(session?.autoCompactAt)
  const cliLimit = compactLimit ? 0 : isLiveSegment ? valid(session?.contextWindowSize) : 0
  // codex reports its model's window with each response: exact, not a guess.
  const engineLimit = compactLimit || cliLimit ? 0 : valid(isLiveSegment && latestUsage ? latestWindow : peakWindow)
  // Оценка по имени: у сессии ДВА источника модели, и легаси может застрять в
  // ЛЮБОМ из них (витки хранят opus-4-8 до апгрейда, а таб уже живой opus-5
  // после hot-switch — инцидент «100% при 510K»). Берём максимум окон обоих.
  const tabLimit = tabModel ? contextLimitForModel(tabModel) : 0
  const jsonlLimit = useModel ? contextLimitForModel(useModel) : 0
  const limit = compactLimit || cliLimit || engineLimit || Math.max(tabLimit, jsonlLimit) || contextLimitForModel(null)
  const modelForLimit = engineLimit
    ? (useModel ?? tabModel ?? '')
    : ((tabLimit >= jsonlLimit ? tabModel : useModel) ?? useModel ?? tabModel ?? '')
  let used: number | null = useUsedTokens >= 0 ? useUsedTokens : (useUsage ? effectiveInputTokens(useUsage) : null)
  let freshness: ContextStats['freshness'] = 'usage'
  if (isLiveSegment && lastCompact && !latestUsage) {
    const post = lastCompact.postTokens
    used = typeof post === 'number' && Number.isSafeInteger(post) && post >= 0 ? post : null
    freshness = used === null ? 'pending' : 'compact'
  }
  const pct = used === null ? null : Math.min(100, Math.round((used / limit) * 100))
  return {
    pct,
    used,
    limit,
    limitSource: compactLimit ? 'compact' : cliLimit ? 'cli' : engineLimit ? 'engine' : 'model',
    cost: cumulativeCost,
    codexCost,
    codexUnpricedTokens,
    model: modelForLimit,
    freshness,
  }
}

/** The chip's cost text and tooltip. Claude responses show their API cost;
 *  ChatGPT responses are billed by the ChatGPT plan, so they show only the
 *  API-equivalent list price, and tokens where no price is published. */
export function costDisplay(s: Pick<ContextStats, 'cost' | 'codexCost' | 'codexUnpricedTokens'>): {
  text: string
  tooltip: string
} {
  if (s.codexCost === null) return { text: formatCost(s.cost), tooltip: 'Session cost (estimate)' }
  const parts: string[] = []
  const notes: string[] = []
  const claude = s.cost > 0
  if (claude) {
    parts.push(formatCost(s.cost))
    notes.push(`Claude ${formatCost(s.cost)} (API estimate)`)
  }
  const priced = s.codexCost > 0 || s.codexUnpricedTokens === 0
  if (priced) {
    parts.push(`≈${formatCost(s.codexCost)} API`)
    notes.push(
      `ChatGPT ≈${formatCost(s.codexCost)}: billed by the ChatGPT plan; this is the API-equivalent price ` +
        `(OpenAI list prices as of ${OPENAI_PRICES_AS_OF})`,
    )
  }
  if (s.codexUnpricedTokens > 0) {
    const tokens = formatTokenCount(s.codexUnpricedTokens)
    parts.push(`${tokens} tok`)
    notes.push(`${tokens} ChatGPT tokens of a model without a published API price`)
  }
  return { text: parts.join(' + '), tooltip: notes.join(' · ') }
}

function formatTokenCount(n: number): string {
  if (n >= 1_000_000) return (n / 1_000_000).toFixed(1) + 'M'
  if (n >= 1_000) return (n / 1_000).toFixed(1) + 'K'
  return String(n)
}
