// vscode DocumentSelector scoring (faithful to vscode `languageSelector.ts#score`):
// an exact language/scheme match scores 10, a `*` wildcard 5, a declared-but-
// mismatched field disqualifies the whole filter (0). `scoreSelector` IS
// `vscode.languages.match`; `matchesSelector` is the boolean (`score > 0`) the
// language-feature provider registry gates on — one source of truth so they
// never drift. Split from language-features.ts to hold the 250-LOC ceiling.
import type { DocumentSelector } from "./language-feature-types.js"

// Every KaminIDE editor document is file-backed (the renderer creates Monaco
// models via Uri.file), so the doc scheme is "file". When non-file documents
// (untitled, virtual) land, thread the real scheme through the provide* calls.
export const DEFAULT_DOC_SCHEME = "file"

const SCORE_EXACT = 10 // exact language/scheme match
const SCORE_WILDCARD = 5 // `*` wildcard match

function scoreOne(s: unknown, languageId: string, scheme: string): number {
  if (typeof s === "string") return s === "*" ? SCORE_WILDCARD : s === languageId ? SCORE_EXACT : 0
  if (s && typeof s === "object") {
    const f = s as { language?: unknown; scheme?: unknown }
    let ret = 0
    if (typeof f.scheme === "string") {
      if (f.scheme === scheme) ret = SCORE_EXACT
      else if (f.scheme === "*") ret = Math.max(ret, SCORE_WILDCARD)
      else return 0
    }
    if (typeof f.language === "string") {
      if (f.language === languageId) ret = SCORE_EXACT
      else if (f.language === "*") ret = Math.max(ret, SCORE_WILDCARD)
      else return 0
    }
    // A filter that declared neither language nor scheme (only a `pattern`, which
    // we don't glob here) is treated as a permissive match so pattern-only
    // selectors still sync — matches the prior lenient behaviour.
    return typeof f.language !== "string" && typeof f.scheme !== "string" ? SCORE_EXACT : ret
  }
  return 0
}

/** vscode.languages.match — the best score across a DocumentSelector. */
export function scoreSelector(selector: DocumentSelector, languageId: string, scheme: string = DEFAULT_DOC_SCHEME): number {
  const list: unknown[] = Array.isArray(selector) ? selector : [selector]
  let best = 0
  for (const s of list) {
    const v = scoreOne(s, languageId, scheme)
    if (v === SCORE_EXACT) return SCORE_EXACT // can't beat a full match
    if (v > best) best = v
  }
  return best
}

export function matchesSelector(selector: DocumentSelector, languageId: string, scheme: string = DEFAULT_DOC_SCHEME): boolean {
  return scoreSelector(selector, languageId, scheme) > 0
}
