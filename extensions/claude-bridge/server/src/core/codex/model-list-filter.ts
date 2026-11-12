// ============================================================================
// Which GPT models Bridge offers: the current generation only.
//
// codex `model/list` also reports older models (owner, 2026-11-12: «старые
// гпт не нужны»). The rule is general, not a list of names, so a new catalog
// needs no Bridge change:
//   1. `hidden` models are never offered;
//   2. a model whose `upgrade` (or `upgradeInfo.model`) names another listed
//      model is superseded by it;
//   3. of the `gpt-<major>[.<minor>][-<tier>]` slugs only the newest major
//      generation stays, and within one tier (Sol, Astra, Luna, …) only its
//      newest minor version.
// A slug outside that scheme is kept: Bridge cannot place it in a generation.
// The server filters, so the chat selector, the main-agent choice and a model
// change all see the same list.
// ============================================================================

export interface ListedModel {
  id: string
  model: string
  hidden?: boolean
  upgrade?: string | null
  upgradeInfo?: { model?: string | null } | null
  isDefault: boolean
}

export interface ModelGeneration {
  major: number
  minor: number
  /** Lower-case tier name; '' for a slug without one (`gpt-5.5`). */
  tier: string
}

const SLUG = /^gpt-(\d+)(?:\.(\d+))?(?:-([a-z][a-z0-9]*))?$/i

export function modelGeneration(slug: string): ModelGeneration | null {
  const m = SLUG.exec(slug.trim())
  if (!m) return null
  return { major: Number(m[1]), minor: m[2] === undefined ? 0 : Number(m[2]), tier: (m[3] ?? '').toLowerCase() }
}

const slugOf = (m: ListedModel): string => m.model || m.id

function upgradeTarget(m: ListedModel): string | null {
  return m.upgrade || m.upgradeInfo?.model || null
}

/** The models Bridge offers, in the reported order; exactly one stays the
 *  default when the reported default is dropped (its upgrade, else the first). */
export function currentModels<T extends ListedModel>(models: T[]): T[] {
  const visible = models.filter((m) => !m.hidden)
  const listed = new Set(visible.flatMap((m) => [m.id, m.model]))
  const fresh = visible.filter((m) => {
    const to = upgradeTarget(m)
    return !(to && to !== m.id && to !== m.model && listed.has(to))
  })

  const generations = new Map(fresh.map((m) => [m, modelGeneration(slugOf(m))] as const))
  let newestMajor = -1
  for (const g of generations.values()) if (g && g.major > newestMajor) newestMajor = g.major
  const newestMinor = new Map<string, number>()
  for (const g of generations.values()) {
    if (!g || g.major !== newestMajor) continue
    newestMinor.set(g.tier, Math.max(newestMinor.get(g.tier) ?? -1, g.minor))
  }
  const kept = fresh.filter((m) => {
    const g = generations.get(m)
    return !g || (g.major === newestMajor && g.minor === newestMinor.get(g.tier))
  })

  if (kept.length === 0 || kept.some((m) => m.isDefault)) return kept
  const dropped = models.find((m) => m.isDefault)
  const target = dropped ? upgradeTarget(dropped) : null
  const heir = kept.find((m) => target && (m.id === target || m.model === target)) ?? kept[0]!
  return kept.map((m) => (m === heir ? { ...m, isDefault: true } : m))
}
