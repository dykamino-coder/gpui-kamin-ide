/** Canonical workspace-shell palette. Desktop terminals may override the background. */
export const DARK_TERMINAL_PALETTE = {
  foreground: '#cfd4e2', cursor: '#f5e0dc',
  selectionBackground: '#515567', selectionForeground: '#cfd4e2',
  black: '#515567', red: '#f38ba8', green: '#a6e3a1', yellow: '#f9e2af',
  blue: '#89b4fa', magenta: '#f5c2e7', cyan: '#94e2d5', white: '#afb6ca',
  brightBlack: '#60667b', brightRed: '#f38ba8', brightGreen: '#a6e3a1', brightYellow: '#f9e2af',
  brightBlue: '#89b4fa', brightMagenta: '#f5c2e7', brightCyan: '#94e2d5', brightWhite: '#adb3c7',
} as const

const ANSI_KEYS = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white',
  'brightBlack', 'brightRed', 'brightGreen', 'brightYellow', 'brightBlue', 'brightMagenta', 'brightCyan', 'brightWhite'] as const
const hex = (value: number) => value.toString(16).padStart(2, '0')
const rgb = (color: string) => 'rgb:' + [1, 3, 5].map(i => color.slice(i, i + 2).repeat(2)).join('/')
const osc = (id: number, value = '') => `\x1b]${id}${value ? ';' + value : ''}\x1b\\`
const DEFAULT_ANSI: readonly string[] = [
  ...ANSI_KEYS.map(key => DARK_TERMINAL_PALETTE[key]),
  ...Array.from({ length: 216 }, (_, i) => '#' + [Math.floor(i / 36), Math.floor(i / 6) % 6, i % 6]
    .map(n => hex(n === 0 ? 0 : 55 + n * 40)).join('')),
  ...Array.from({ length: 24 }, (_, i) => '#' + hex(8 + i * 10).repeat(3)),
]
const DEFAULT_DYNAMIC = [DARK_TERMINAL_PALETTE.foreground, '#0b0c0f', DARK_TERMINAL_PALETTE.cursor]

/** X11 hexadecimal forms, at the renderer's 8-bit channel precision. */
function parseColor(value: string): string | undefined {
  let parts: string[] | undefined
  if (/^#[\da-f]{3}(?:[\da-f]{3}){0,3}$/i.test(value)) {
    const width = (value.length - 1) / 3
    parts = [0, 1, 2].map(i => value.slice(1 + i * width, 1 + (i + 1) * width))
  } else if (/^rgb:[\da-f]{1,4}\/[\da-f]{1,4}\/[\da-f]{1,4}$/i.test(value)) parts = value.slice(4).split('/')
  if (!parts) return
  return '#' + parts.map(part => part.length === 1 ? part.repeat(2) : part.slice(0, 2)).join('').toLowerCase()
}

interface Disposable { dispose(): void }
interface Parser {
  registerOscHandler(id: number, handler: (value: string) => boolean): Disposable
  registerEscHandler(id: { final: string }, handler: () => boolean): Disposable
}

/** One implementation for the authoritative parser and silent browser replicas.
 * Queries never change state; set/reset sequences update a bounded 256-color table.
 */
export class TerminalColors {
  private ansi = [...DEFAULT_ANSI]
  private dynamic: string[] = [...DEFAULT_DYNAMIC]

  theme() {
    return { ...DARK_TERMINAL_PALETTE, ...Object.fromEntries(ANSI_KEYS.map((key, i) => [key, this.ansi[i]!])),
      extendedAnsi: this.ansi.slice(16), foreground: this.dynamic[0]!, background: this.dynamic[1]!, cursor: this.dynamic[2]! }
  }

  attach(parser: Parser, respond: (data: string) => void, changed: () => void = () => {}): () => void {
    const hooks: Disposable[] = []
    const register = (id: number, handler: (value: string) => void) => {
      hooks.push(parser.registerOscHandler(id, value => { handler(value); return true }))
    }
    register(4, value => {
      const parts = value.split(';')
      let dirty = false
      for (let i = 0; i + 1 < parts.length; i += 2) {
        const index = this.index(parts[i]!)
        if (index === undefined) continue
        if (parts[i + 1] === '?') respond(osc(4, `${index};${rgb(this.ansi[index]!)}`))
        else {
          const color = parseColor(parts[i + 1]!)
          if (color) { this.ansi[index] = color; dirty = true }
        }
      }
      if (dirty) changed()
    })
    register(104, value => {
      if (!value) this.ansi = [...DEFAULT_ANSI]
      else for (const part of value.split(';')) {
        const index = this.index(part)
        if (index !== undefined) this.ansi[index] = DEFAULT_ANSI[index]!
      }
      changed()
    })
    for (const id of [10, 11, 12]) {
      register(id, value => {
        let dirty = false
        for (const [offset, part] of value.split(';').slice(0, 13 - id).entries()) {
          const index = id - 10 + offset
          if (part === '?') respond(osc(id + offset, rgb(this.dynamic[index]!)))
          else {
            const color = parseColor(part)
            if (color) { this.dynamic[index] = color; dirty = true }
          }
        }
        if (dirty) changed()
      })
      register(id + 100, () => { this.dynamic[id - 10] = DEFAULT_DYNAMIC[id - 10]!; changed() })
    }
    hooks.push(parser.registerEscHandler({ final: 'c' }, () => {
      this.ansi = [...DEFAULT_ANSI]; this.dynamic = [...DEFAULT_DYNAMIC]; changed()
      return false // Let xterm reset its screen and input modes too.
    }))
    return () => { for (const hook of hooks) hook.dispose() }
  }

  /** Reset a returning replica before restoring mutations; contains no query. */
  snapshot(): string {
    let output = osc(104) + osc(110) + osc(111) + osc(112)
    for (const [index, color] of this.ansi.entries()) if (color !== DEFAULT_ANSI[index]) output += osc(4, `${index};${color}`)
    for (const [index, color] of this.dynamic.entries()) if (color !== DEFAULT_DYNAMIC[index]) output += osc(10 + index, color)
    return output
  }

  private index(value: string): number | undefined {
    if (!/^\d{1,3}$/.test(value)) return
    const index = Number(value)
    return index <= 255 ? index : undefined
  }
}
