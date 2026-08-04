import { describe, it, expect } from 'vitest'
import { trimHeavyToolResults } from './heavy-tool-result'
import type { JsonlEntryData, ContentBlock } from '../types/jsonl'

const PREFIX = 16_384
const big = (n: number, ch = 'x') => ch.repeat(n)

const entry = (blocks: ContentBlock[]): JsonlEntryData =>
  ({ type: 'user', uuid: 'u', message: { content: blocks } } as unknown as JsonlEntryData)

const firstBlock = (e: JsonlEntryData) => (e.message!.content as ContentBlock[])[0]!

describe('trimHeavyToolResults', () => {
  it('trims a fat string body to the prefix and records the true length', () => {
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: big(400_000) }])
    trimHeavyToolResults(e)
    const b = firstBlock(e)
    expect((b.content as string).length).toBe(PREFIX)
    expect(b._fullLen).toBe(400_000)
  })

  it('leaves a body at or under the threshold untouched', () => {
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: big(5_000) }])
    trimHeavyToolResults(e)
    expect((firstBlock(e).content as string).length).toBe(5_000)
    expect(firstBlock(e)._fullLen).toBeUndefined()
  })

  it('NEVER truncates a base64 image data URI, however large', () => {
    const uri = 'data:image/png;base64,' + big(500_000, 'A')
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: uri }])
    trimHeavyToolResults(e)
    expect((firstBlock(e).content as string).length).toBe(uri.length)
    expect(firstBlock(e)._fullLen).toBeUndefined()
  })

  it('NEVER truncates a JSON-wrapped image payload', () => {
    const json = JSON.stringify([{ type: 'image', source: { type: 'base64', media_type: 'image/png', data: big(500_000, 'A') } }])
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: json }])
    trimHeavyToolResults(e)
    expect((firstBlock(e).content as string).length).toBe(json.length)
  })

  it('trims large TEXT items in structured content, leaving image items whole', () => {
    const imgData = big(300_000, 'A')
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: [
      { type: 'text', text: big(400_000) } as ContentBlock,
      { type: 'image', source: { type: 'base64', data: imgData } } as unknown as ContentBlock,
    ] }])
    trimHeavyToolResults(e)
    const arr = firstBlock(e).content as ContentBlock[]
    expect(arr[0]!.text!.length).toBe(PREFIX)
    expect((arr[1] as any).source.data.length).toBe(300_000) // image untouched
    expect(firstBlock(e)._fullLen).toBe(400_000)
  })

  it('is idempotent — a second pass does not re-trim or clobber _fullLen', () => {
    const e = entry([{ type: 'tool_result', tool_use_id: 't', content: big(400_000) }])
    trimHeavyToolResults(e)
    trimHeavyToolResults(e)
    expect((firstBlock(e).content as string).length).toBe(PREFIX)
    expect(firstBlock(e)._fullLen).toBe(400_000)
  })

  it('ignores non-tool_result blocks and non-array message content', () => {
    const e = entry([{ type: 'text', text: big(400_000) }])
    trimHeavyToolResults(e)
    expect(firstBlock(e).text!.length).toBe(400_000) // assistant/text bodies not our target
  })
})
