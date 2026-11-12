import { describe, expect, it, vi } from 'vitest'
import {
  buildPatchedCatalog,
  isCatalogLockedDown,
  parseCatalogOutput,
  patchModelCatalog,
  validateCatalog,
} from './model-catalog'

// Shape of `codex debug models --bundled` (0.160.1), trimmed to the fields that matter.
const bundled = {
  models: [
    {
      slug: 'gpt-6-sol',
      apply_patch_tool_type: 'freeform',
      shell_type: 'unified_exec',
      experimental_supported_tools: ['send_user_message_async', 'clock'],
      multi_agent_version: 'v2',
      tool_mode: 'code_mode_only',
    },
    {
      slug: 'gpt-5.5',
      apply_patch_tool_type: 'freeform',
      shell_type: 'unified_exec',
      experimental_supported_tools: [],
      multi_agent_version: null,
    },
  ],
}

describe('model catalog', () => {
  it('parses CLI output with a BOM and WARNING lines', () => {
    const text = '﻿WARNING: proceeding, even though we could not create PATH aliases\r\n' + JSON.stringify(bundled)
    expect(parseCatalogOutput(text).models.map((m) => m.slug)).toEqual(['gpt-6-sol', 'gpt-5.5'])
  })

  it('removes every built-in tool selector from every model without mutating the input', () => {
    const before = structuredClone(bundled)
    const patched = patchModelCatalog(bundled)
    expect(bundled).toEqual(before)
    expect(isCatalogLockedDown(bundled)).toBe(false)
    expect(isCatalogLockedDown(patched)).toBe(true)
    for (const m of patched.models) {
      expect(m).toMatchObject({
        apply_patch_tool_type: null,
        shell_type: 'disabled',
        experimental_supported_tools: [],
        multi_agent_version: 'disabled',
      })
    }
    expect(patched.models[0]!.tool_mode).toBe('code_mode_only')
  })

  it('rejects catalogs that are not catalogs', () => {
    expect(() => validateCatalog({})).toThrow('no models array')
    expect(() => validateCatalog({ models: [] })).toThrow('empty')
    expect(() => validateCatalog({ models: [{ slug: '' }] })).toThrow('without slug')
    expect(() => parseCatalogOutput('not json')).toThrow()
  })

  it('builds from the pinned binary through the injected runner', async () => {
    const run = vi.fn(async () => JSON.stringify(bundled))
    const catalog = await buildPatchedCatalog(run)
    expect(run).toHaveBeenCalledWith(['debug', 'models', '--bundled'])
    expect(isCatalogLockedDown(catalog)).toBe(true)
  })
})
