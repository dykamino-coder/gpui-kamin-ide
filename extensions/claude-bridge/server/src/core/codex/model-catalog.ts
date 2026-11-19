// ============================================================================
// Patched codex model catalog.
//
// apply_patch and the shell are chosen per model by the catalog, not by
// feature flags (codex core/src/tools/spec_plan.rs), so Bridge feeds codex its
// own copy of the bundled catalog via `model_catalog_json` with those tools
// removed from every model. An explicit catalog also stops codex from fetching
// the server-side one, so the catalog must be rebuilt from
// `codex debug models --bundled` of the pinned binary on every version change.
// ============================================================================

export interface CatalogModel {
  slug: string
  display_name?: string
  visibility?: string
  apply_patch_tool_type?: string | null
  shell_type?: string
  experimental_supported_tools?: string[]
  multi_agent_version?: string | null
  [key: string]: unknown
}

export interface ModelCatalog {
  models: CatalogModel[]
  [key: string]: unknown
}

/** Fields that select a built-in tool, and the value that removes it. */
export const CATALOG_TOOL_PATCH = {
  apply_patch_tool_type: null,
  shell_type: 'disabled',
  experimental_supported_tools: [] as string[],
  // The config switch alone (`features.multi_agent = false` before 0.168,
  // `agents.enabled = false` now) is not trusted to stop a catalog-declared v2
  // runtime: code-mode models once got spawn_agent & co. in a `collaboration`
  // namespace.
  multi_agent_version: 'disabled',
} as const

/**
 * Fields that decide whether a turn streams reasoning. With Responses Lite
 * (`use_responses_lite: true`, the bundled default of every GPT-6 model in codex
 * 0.170.0) the backend returns no reasoning item at all, whatever summary the
 * turn asks for: `codex exec` with effort `high` and summary `detailed` reported
 * `reasoning_output_tokens: 0` and no reasoning event, and the same request
 * without Lite streamed a reasoning summary. The catalog default summary is
 * `none`; `auto` matches what `turn/start` asks for. (The rollout
 * `turn_context.summary` is always written as `none` and proves nothing.)
 */
export const CATALOG_REASONING_PATCH = {
  use_responses_lite: false,
  default_reasoning_summary: 'auto',
} as const

/**
 * `codex debug models` prints `WARNING:` diagnostics on stdout before the
 * JSON on some platforms (e.g. PATH-alias warnings on Windows temp homes).
 */
export function parseCatalogOutput(text: string): ModelCatalog {
  const body = text
    .replace(/^\uFEFF/, '')
    .split(/\r?\n/)
    .filter((line) => !line.startsWith('WARNING'))
    .join('\n')
  const parsed: unknown = JSON.parse(body)
  return validateCatalog(parsed)
}

export function validateCatalog(value: unknown): ModelCatalog {
  if (!value || typeof value !== 'object' || !Array.isArray((value as ModelCatalog).models)) {
    throw new Error('Codex model catalog has no models array')
  }
  const catalog = value as ModelCatalog
  if (catalog.models.length === 0) throw new Error('Codex model catalog is empty')
  for (const model of catalog.models) {
    if (!model || typeof model.slug !== 'string' || !model.slug) throw new Error('Codex catalog model without slug')
  }
  return catalog
}

/** Returns a patched deep copy; the input is left untouched. */
export function patchModelCatalog(catalog: ModelCatalog): ModelCatalog {
  const copy = structuredClone(validateCatalog(catalog))
  for (const model of copy.models) {
    model.apply_patch_tool_type = CATALOG_TOOL_PATCH.apply_patch_tool_type
    model.shell_type = CATALOG_TOOL_PATCH.shell_type
    model.experimental_supported_tools = [...CATALOG_TOOL_PATCH.experimental_supported_tools]
    model.multi_agent_version = CATALOG_TOOL_PATCH.multi_agent_version
    model.use_responses_lite = CATALOG_REASONING_PATCH.use_responses_lite
    model.default_reasoning_summary = CATALOG_REASONING_PATCH.default_reasoning_summary
  }
  return copy
}

/** True when every model in the catalog has its built-in tools removed. */
export function isCatalogLockedDown(catalog: ModelCatalog): boolean {
  return catalog.models.every(
    (m) =>
      m.apply_patch_tool_type === null &&
      m.shell_type === CATALOG_TOOL_PATCH.shell_type &&
      m.multi_agent_version === CATALOG_TOOL_PATCH.multi_agent_version &&
      Array.isArray(m.experimental_supported_tools) &&
      m.experimental_supported_tools.length === 0,
  )
}

/** `run(args)` executes the pinned codex binary and resolves with its stdout. */
export async function buildPatchedCatalog(run: (args: string[]) => Promise<string>): Promise<ModelCatalog> {
  return patchModelCatalog(parseCatalogOutput(await run(['debug', 'models', '--bundled'])))
}
