// Incremental Program Status Protocol parser. Terminal output is never consumed.
export type ProgramStatus = 'working' | 'blocked' | 'done' | 'idle' | 'error' | 'clear'

export class ProgramStatusParser {
  private pending = ''
  constructor(
    private readonly status: (state: ProgramStatus) => void,
    private readonly reply: () => void,
  ) {}

  feed(chunk: string): void {
    // Scan incrementally so a split ESC, OSC introducer or ST is retained.
    for (const char of chunk) {
      if (!this.pending) {
        if (char === '\x1b') this.pending = char
        continue
      }
      if (this.pending === '\x1b') {
        this.pending = char === ']' ? '\x1b]' : char === '\x1b' ? char : ''
        continue
      }
      this.pending += char
      if (char === '\x07' || this.pending.endsWith('\x1b\\')) {
        const body = this.pending.slice(2, char === '\x07' ? -1 : -2)
        this.pending = ''
        if (!body.startsWith('7501;')) continue
        const data = body.slice(5)
        if (data === '?') {
          this.reply()
          continue
        }
        const fields = new Map(
          data.split(':').map((field) => {
            const equal = field.indexOf('=')
            return [field.slice(0, equal), field.slice(equal + 1)]
          }),
        )
        if (fields.has('id')) continue
        const state = fields.get('state')
        if (state === 'clear') this.status('clear')
        else if (
          fields.get('app') === 'claude-code' &&
          (state === 'working' || state === 'blocked' || state === 'done' || state === 'idle' || state === 'error')
        ) {
          this.status(state)
        }
      } else if (this.pending.length > 8192) this.pending = ''
    }
  }
}
