// Workspace-symbol registry (Go to Symbol in Workspace, Ctrl+T). Kept separate
// from LanguageFeatures because these providers are NOT per-document — no
// selector, no docFor, just a query merged across all providers. Also keeps
// language-features.ts under the 250-LOC ceiling.
import type { WorkspaceSymbolDto } from "../../api/language-feature-dtos.js"
import type { Disposable as DisposableType } from "../../api/types.js"
import { toWorkspaceSymbolDtos } from "./language-feature-dtos-nav.js"
import type { WorkspaceSymbolProvider } from "./language-feature-types.js"
import { rangeDto } from "./range-dto.js"
import { Disposable } from "./shared.js"

const NO_TOKEN = { isCancellationRequested: false, onCancellationRequested: () => ({ dispose() { /* */ } }) }

/** A symbol is navigable only if its location carries a usable range. */
function hasRange(symbol: unknown): boolean {
  const loc = (symbol as { location?: { range?: unknown } } | null)?.location
  return loc !== undefined && rangeDto(loc.range) !== undefined
}

/** Fill in range-less symbols via the provider's resolveWorkspaceSymbol. VS Code
 *  resolves lazily on selection; we resolve eagerly (result sets are small) so
 *  toWorkspaceSymbolDtos doesn't drop a symbol that only lacked a range. */
async function resolveRanges(provider: WorkspaceSymbolProvider, raw: unknown): Promise<unknown[]> {
  if (!Array.isArray(raw)) return []
  const arr = raw as unknown[]
  if (!provider.resolveWorkspaceSymbol) return arr
  const out: unknown[] = []
  for (const sym of arr) {
    if (sym && typeof sym === "object" && !hasRange(sym)) {
      try { out.push((await Promise.resolve(provider.resolveWorkspaceSymbol(sym, NO_TOKEN))) ?? sym) }
      catch { out.push(sym) }
    } else out.push(sym)
  }
  return out
}

export class WorkspaceSymbolRegistry {
  private readonly providers: WorkspaceSymbolProvider[] = []

  register(provider: WorkspaceSymbolProvider): DisposableType {
    this.providers.push(provider)
    return new Disposable(() => { const i = this.providers.indexOf(provider); if (i >= 0) this.providers.splice(i, 1) })
  }

  /** Query merged across all registered providers (none are document-scoped).
   *  Range-less symbols are resolved before mapping so they aren't dropped. */
  async provide(query: string): Promise<WorkspaceSymbolDto[]> {
    const out: WorkspaceSymbolDto[] = []
    for (const provider of this.providers) {
      try {
        const raw = await Promise.resolve(provider.provideWorkspaceSymbols(query, NO_TOKEN))
        out.push(...toWorkspaceSymbolDtos(await resolveRanges(provider, raw)))
      } catch (err) {
        console.warn("language-features: workspace-symbol provider threw:", err)
      }
    }
    return out
  }
}
