// ============================================================================
// OpenAI API list prices for the models a codex (ChatGPT) chat can run.
//
// Bridge signs codex in with a ChatGPT account, so these tokens are billed by
// the subscription, not per token. The dashboard and the chat show the price
// only as an "API equivalent": what the same tokens would cost on the OpenAI
// API. A model missing here has NO price: callers show its tokens without $,
// never a guess and never a Claude rate.
//
// Source: https://developers.openai.com/api/docs/pricing (standard tier,
// per 1M tokens), read 2026-11-12. Long-context rates apply to the whole
// request once its input exceeds 272K tokens; the page lists the long cache
// write rate only for some models, so the long tier doubles the short one
// (the page's rule for input and cache prices). Cache write uses the 30-minute
// rate: codex reports cache writes without their retention.
// Shared by the server (dashboard) and the webview (chat cost bar).
// ============================================================================

export const OPENAI_PRICES_SOURCE = 'https://developers.openai.com/api/docs/pricing'
export const OPENAI_PRICES_AS_OF = '2026-11-12'
export const OPENAI_LONG_CONTEXT_TOKENS = 272_000

export interface OpenAiRate {
  input: number
  /** null: the model has no cached-input price (cached tokens bill as input). */
  cachedInput: number | null
  cacheWrite: number | null
  output: number
  /** Rates of a request above OPENAI_LONG_CONTEXT_TOKENS input tokens. */
  long?: { input: number; cachedInput: number | null; output: number }
}

function rate(
  input: number,
  cachedInput: number | null,
  cacheWrite: number | null,
  output: number,
  long?: [number, number | null, number],
): OpenAiRate {
  return {
    input,
    cachedInput,
    cacheWrite,
    output,
    ...(long ? { long: { input: long[0], cachedInput: long[1], output: long[2] } } : {}),
  }
}

const RATES: Record<string, OpenAiRate> = {
  'gpt-6-astra': rate(10, 1, 12.5, 50, [20, 2, 75]),
  'gpt-6.1-sol': rate(2, 0.1, 2.5, 10, [4, 0.2, 15]),
  'gpt-6.1-terra': rate(1, 0.05, 1.25, 5, [2, 0.1, 7.5]),
  'gpt-6-luna': rate(0.1, 0.01, 0.125, 0.5, [0.2, 0.02, 0.75]),
  'gpt-6-sol': rate(2, 0.2, 2.5, 10, [4, 0.4, 15]),
  'gpt-5.6-sol': rate(4, 0.4, 5, 20, [8, 0.8, 30]),
  'gpt-5.6-terra': rate(2, 0.2, 2.5, 12, [4, 0.4, 18]),
  'gpt-5.6-luna': rate(0.2, 0.02, 0.25, 1.2, [0.4, 0.04, 1.8]),
  'gpt-5.5': rate(5, 0.5, null, 30, [10, 1, 45]),
  'gpt-5.5-pro': rate(30, null, null, 180, [60, null, 270]),
  'gpt-5.4': rate(2.5, 0.25, null, 15, [5, 0.5, 22.5]),
  'gpt-5.4-pro': rate(30, null, null, 180, [60, null, 270]),
  'gpt-5.4-mini': rate(0.75, 0.075, null, 4.5),
  'gpt-5.4-nano': rate(0.2, 0.02, null, 1.25),
  'gpt-5.2': rate(1.75, 0.175, null, 14),
  'gpt-5.2-pro': rate(21, null, null, 168),
  'gpt-5.1': rate(1.25, 0.125, null, 10),
  'gpt-5': rate(1.25, 0.125, null, 10),
  'gpt-5-mini': rate(0.25, 0.025, null, 2),
  'gpt-5-nano': rate(0.05, 0.005, null, 0.4),
  'gpt-5-pro': rate(15, null, null, 120),
  'gpt-4.1': rate(2, 0.5, null, 8),
  'gpt-4.1-mini': rate(0.4, 0.1, null, 1.6),
  'gpt-4o': rate(2.5, 1.25, null, 10),
  'gpt-4o-mini': rate(0.15, 0.075, null, 0.6),
  o3: rate(2, 0.5, null, 8),
  'o3-pro': rate(20, null, null, 80),
}

/** Exact list price of a model id (`openai/` prefix and case ignored), or null. */
export function openAiRateFor(model: string | null | undefined): OpenAiRate | null {
  const id = (model ?? '')
    .trim()
    .toLowerCase()
    .replace(/^openai\//, '')
  return Object.prototype.hasOwnProperty.call(RATES, id) ? RATES[id]! : null
}

/** One API response's tokens; `input` excludes cached reads and cache writes. */
export interface EngineTokens {
  input: number
  cacheRead: number
  cacheWrite: number
  output: number
}

/** Whether ONE request is billed at the long-context rate. */
export function isLongOpenAiRequest(t: EngineTokens): boolean {
  return t.input + t.cacheRead + t.cacheWrite > OPENAI_LONG_CONTEXT_TOKENS
}

/** API-equivalent USD of ONE request, or null when the model has no list price. */
export function openAiRequestCost(model: string | null | undefined, t: EngineTokens): number | null {
  return openAiCost(model, t, isLongOpenAiRequest(t))
}

/** API-equivalent USD of tokens summed over requests of one rate tier
 *  (`longContext` = they were all long-context requests); null = no price. */
export function openAiCost(model: string | null | undefined, t: EngineTokens, longContext: boolean): number | null {
  const r = openAiRateFor(model)
  if (!r) return null
  const long = longContext && r.long ? r.long : null
  const input = long ? long.input : r.input
  const cached = long ? long.cachedInput : r.cachedInput
  const write = r.cacheWrite === null ? null : long ? r.cacheWrite * 2 : r.cacheWrite
  return (
    (t.input * input +
      t.cacheRead * (cached ?? input) +
      t.cacheWrite * (write ?? input) +
      t.output * (long ? long.output : r.output)) /
    1_000_000
  )
}
