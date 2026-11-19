// CLI 2.1.328 adds an `api-request-resumed` bookkeeping row (a sibling of
// `api-request`/`-shape`/`-blob`) and a `tool_offer_record` model-only
// attachment. Neither may draw a chat row.
import { describe, expect, it } from 'vitest'
import type { JsonlEntryData } from '../../types/jsonl'
import { isVisibleAttachment, RENDERABLE_ENTRY_TYPES } from './utils'

describe('CLI 2.1.328 transcript rows', () => {
  it('has no rendering branch for the api-request family', () => {
    for (const type of ['api-request', 'api-request-shape', 'api-request-blob', 'api-request-resumed']) {
      expect(RENDERABLE_ENTRY_TYPES.has(type)).toBe(false)
    }
  })

  it('keeps the new attachment kinds invisible', () => {
    for (const kind of ['tool_offer_record', 'prompt_snapshot', 'prompt_render_point']) {
      const row = { type: 'attachment', uuid: kind, attachment: { type: kind } } as unknown as JsonlEntryData
      expect(isVisibleAttachment(row)).toBe(false)
    }
  })
})
