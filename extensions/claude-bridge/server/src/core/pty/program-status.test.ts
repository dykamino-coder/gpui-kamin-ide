import { describe, expect, it, vi } from 'vitest'
import { ProgramStatusParser } from './program-status'

describe('OSC 7501 incremental parser', () => {
  for (const terminator of ['\x07', '\x1b\\']) {
    const frame = `\x1b]7501;state=blocked:app=claude-code:kind=permission:msg=YWJj${terminator}`
    it(`parses every split with ${JSON.stringify(terminator)}`, () => {
      for (let split = 0; split <= frame.length; split++) {
        const states: string[] = []
        const parser = new ProgramStatusParser(
          (s) => states.push(s),
          () => {},
        )
        parser.feed('ordinary output' + frame.slice(0, split))
        parser.feed(frame.slice(split))
        expect(states).toEqual(['blocked'])
      }
    })
  }
  it('answers capability queries, ignores child statuses and preserves clear semantics', () => {
    const states: string[] = []
    const reply = vi.fn()
    const parser = new ProgramStatusParser((s) => states.push(s), reply)
    for (const char of '\x1b]7501;?\x1b\\') parser.feed(char)
    parser.feed('\x1b]7501;state=working:app=claude-code:id=worker\x07')
    parser.feed('\x1b]7501;state=clear:id=worker\x07')
    parser.feed('\x1b]7501;state=done:app=other\x07')
    parser.feed('\x1b]7501;state=unknown:app=claude-code\x07')
    parser.feed('\x1b]7501;state=working:app=claude-code\x07\x1b]7501;state=clear\x07')
    expect(reply).toHaveBeenCalledOnce()
    expect(states).toEqual(['working', 'clear'])
  })
  it('bounds unfinished frames and recovers for the next complete frame', () => {
    const states: string[] = []
    const parser = new ProgramStatusParser(
      (s) => states.push(s),
      () => {},
    )
    parser.feed('\x1b]' + 'a'.repeat(100_000))
    parser.feed('\x1b]7501;state=idle:app=claude-code\x07')
    expect(states).toEqual(['idle'])
  })
})
