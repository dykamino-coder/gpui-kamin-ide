// The sweeper decision logic behind the "answer streamed then vanished" fix.
//
// A streaming stub is dropped by sweepOrphanStubs once its TTL expires — but ONLY
// if a later turn superseded it. A tail stub that still carries streamed text is
// kept (its canonical merges when it finally arrives, which can lag well past the
// TTL on a busy session or a deferred CLI flush). These two predicates ARE that
// rule; sweepOrphanStubs is thin glue over them.
import { describe, it, expect } from 'vitest'
import { stubHasContent, stubSuperseded } from './jsonl'
import type { JsonlEntryData } from '../types/jsonl'

type Rec = JsonlEntryData

const stub = (msgId: string, text: string): Rec =>
  ({ type: 'assistant', __streaming: true, message: { id: msgId, content: text ? [{ type: 'text', text }] : [] } } as unknown as Rec)
const committed = (msgId: string, text: string): Rec =>
  ({ type: 'assistant', message: { id: msgId, content: [{ type: 'text', text }] } } as unknown as Rec)
const user = (text: string): Rec =>
  ({ type: 'user', message: { content: [{ type: 'text', text }] } } as unknown as Rec)

describe('stubHasContent', () => {
  it('is true for a stub with non-empty text', () => {
    expect(stubHasContent(stub('m1', 'Hello world'))).toBe(true)
  })
  it('is true for a thinking-only stub', () => {
    expect(stubHasContent({ type: 'assistant', __streaming: true,
      message: { id: 'm', content: [{ type: 'thinking', thinking: 'hmm' }] } } as unknown as Rec)).toBe(true)
  })
  it('is false for an empty stub (block started, no delta yet)', () => {
    expect(stubHasContent(stub('m1', ''))).toBe(false)
    expect(stubHasContent({ type: 'assistant', __streaming: true,
      message: { id: 'm', content: [{ type: 'text', text: '   ' }] } } as unknown as Rec)).toBe(false)
  })
})

describe('stubSuperseded', () => {
  it('a tail stub is NOT superseded — nothing follows it', () => {
    const entries = [user('hi'), stub('m1', 'streaming answer…')]
    expect(stubSuperseded(entries, 1)).toBe(false)
  })

  it('is superseded by a LATER committed assistant with a different msgId (API retry)', () => {
    const entries = [user('hi'), stub('m1', 'partial'), committed('m2', 'the real retried answer')]
    expect(stubSuperseded(entries, 1)).toBe(true)
  })

  it('is NOT superseded by a later committed entry sharing its OWN msgId (its canonical)', () => {
    // Its own canonical arriving as a separate line is a merge target, not a
    // successor — must not count as supersession or the merge race would sweep it.
    const entries = [user('hi'), stub('m1', 'partial'), committed('m1', 'full')]
    expect(stubSuperseded(entries, 1)).toBe(false)
  })

  it('is superseded by a later USER entry (a new turn began)', () => {
    const entries = [user('hi'), stub('m1', 'partial'), user('next question')]
    expect(stubSuperseded(entries, 1)).toBe(true)
  })

  it('is NOT superseded by a later STREAMING stub (same in-flight turn burst)', () => {
    const entries = [user('hi'), stub('m1', 'partial'), stub('m1', 'partial more')]
    expect(stubSuperseded(entries, 1)).toBe(false)
  })
})
