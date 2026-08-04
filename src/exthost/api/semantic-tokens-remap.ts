// Remap a provider's encoded semantic tokens (its own legend) into the shared
// STANDARD legend Monaco is registered with. Tokens whose type isn't in the
// standard legend are dropped — but the LSP delta encoding (deltaLine,
// deltaStartChar relative to the previous token) means we must track absolute
// positions and re-delta the KEPT tokens, or the chain breaks. (B6n)
import type { SemanticTokensDto } from "../../api/language-feature-dtos.js"
import { STANDARD_SEMANTIC_TOKEN_MODIFIERS, STANDARD_SEMANTIC_TOKEN_TYPES } from "../../api/semantic-tokens-legend.js"
import type { SemanticTokensLegendLike } from "./language-feature-types.js"

const TOKEN_STRIDE = 5

export function toSemanticTokensDto(result: unknown, legend: SemanticTokensLegendLike): SemanticTokensDto | null {
  if (!result || typeof result !== "object") return null
  const r = result as { data?: unknown; resultId?: unknown }
  const src = r.data as ArrayLike<number> | undefined
  if (!src || typeof src.length !== "number") return null
  // provider legend index → standard legend index (-1 = not in standard legend).
  const typeMap = legend.tokenTypes.map((name) => STANDARD_SEMANTIC_TOKEN_TYPES.indexOf(name))
  const modMap = legend.tokenModifiers.map((name) => STANDARD_SEMANTIC_TOKEN_MODIFIERS.indexOf(name))
  const out: number[] = []
  let absLine = 0, absChar = 0          // current token's absolute position
  let keptLine = 0, keptChar = 0        // previous EMITTED token's absolute position
  for (let i = 0; i + TOKEN_STRIDE <= src.length; ) {
    // 5-int group: deltaLine, deltaStartChar, length, tokenType, tokenModifiers.
    const dLine = src[i++] ?? 0, dChar = src[i++] ?? 0, len = src[i++] ?? 0
    const type = src[i++] ?? 0, mods = src[i++] ?? 0
    absLine += dLine
    absChar = dLine === 0 ? absChar + dChar : dChar
    const stdType = typeMap[type]
    if (stdType === undefined || stdType < 0) continue // unknown type → drop, but abs pos already advanced
    let stdMods = 0
    for (let b = 0; b < modMap.length; b++) {
      const stdMod = modMap[b]
      if (stdMod !== undefined && stdMod >= 0 && (mods & (1 << b)) !== 0) stdMods |= 1 << stdMod
    }
    const outLine = absLine - keptLine
    out.push(outLine, outLine === 0 ? absChar - keptChar : absChar, len, stdType, stdMods)
    keptLine = absLine; keptChar = absChar
  }
  return typeof r.resultId === "string" ? { data: out, resultId: r.resultId } : { data: out }
}
