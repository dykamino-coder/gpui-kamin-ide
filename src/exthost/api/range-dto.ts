// Defensive normalizer turning an unknown vscode Range-like into the shared
// RangeDto WS shape, used by every language-feature path (completion/hover/
// definition ranges in B6, diagnostic ranges in B6c). The RangeDto type itself
// lives in the shared contract (api/language-feature-dtos.ts) so host + renderer
// share one definition; re-exported here so the many `{ rangeDto, type RangeDto }`
// importers are unaffected.
export type { RangeDto } from "../../api/language-feature-dtos.js"
import type { RangeDto } from "../../api/language-feature-dtos.js"

/** Normalize an unknown vscode Range-like (`{start,end}` of `{line,character}`)
 *  to a 0-based DTO, or undefined if the shape is not a usable range. */
export function rangeDto(r: unknown): RangeDto | undefined {
  if (!r || typeof r !== "object") return undefined
  const range = r as { start?: { line?: unknown; character?: unknown }; end?: { line?: unknown; character?: unknown } }
  if (typeof range.start?.line !== "number" || typeof range.end?.line !== "number") return undefined
  return {
    startLine: range.start.line, startChar: Number(range.start.character ?? 0),
    endLine: range.end.line, endChar: Number(range.end.character ?? 0),
  }
}
