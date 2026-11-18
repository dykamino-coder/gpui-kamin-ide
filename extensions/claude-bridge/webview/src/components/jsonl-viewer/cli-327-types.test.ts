// CLI 2.1.323–2.1.327 add session lineage (`derived-from`) and data-flow
// bookkeeping rows to the transcript, plus more model-only attachments. None of
// them draws a chat row, so none may take a render slot or count toward a
// segment badge.
import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { isVisibleAttachment, NON_RENDERING_ENTRY_TYPES } from './utils'

describe('CLI 2.1.327 transcript rows', () => {
  it('declares the new bookkeeping entry types as non-rendering', () => {
    for (const type of ['derived-from', 'data-flow-records', 'data-flow-incomplete']) {
      expect(NON_RENDERING_ENTRY_TYPES.has(type)).toBe(true)
    }
  })

  it('keeps the new attachment kinds invisible', () => {
    for (const kind of ['agent_ask_notice', 'linked_plugin_tools_note', 'plugin_note', 'tool_results_cut']) {
      const row = { type: 'attachment', uuid: kind, attachment: { type: kind } } as unknown as JsonlEntryData
      expect(isVisibleAttachment(row)).toBe(false)
    }
  })
})
