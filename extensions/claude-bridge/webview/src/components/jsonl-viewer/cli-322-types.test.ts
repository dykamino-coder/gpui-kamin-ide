// CLI 2.1.313–2.1.322 added transcript bookkeeping rows and model-only
// attachments. None of them draws a chat row, so none may take a render slot
// or count toward a segment badge.
import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { isVisibleAttachment, NON_RENDERING_ENTRY_TYPES } from './utils'

describe('CLI 2.1.322 transcript rows', () => {
  it('declares the new bookkeeping entry types as non-rendering', () => {
    for (const type of ['session-owner', 'resume-head-vouch', 'repl-mode']) {
      expect(NON_RENDERING_ENTRY_TYPES.has(type)).toBe(true)
    }
  })

  it('keeps the new and removed attachment kinds invisible', () => {
    for (const kind of ['message_pdf_reads', 'widget_context_hint', 'claude_app_notice', 'file_unchanged', 'output_limit_note']) {
      const row = { type: 'attachment', uuid: kind, attachment: { type: kind } } as unknown as JsonlEntryData
      expect(isVisibleAttachment(row)).toBe(false)
    }
  })
})
