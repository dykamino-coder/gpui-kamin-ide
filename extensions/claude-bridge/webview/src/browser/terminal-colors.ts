import type { Terminal } from '@xterm/xterm'
import { TerminalColors } from '../../shared/terminal-colors'

/** Replicate host color changes without answering queries as keyboard input. */
export function attachTerminalColors(terminal: Pick<Terminal, 'parser' | 'options'>): () => void {
  const colors = new TerminalColors()
  const changed = () => { terminal.options.theme = colors.theme() }
  changed()
  return colors.attach(terminal.parser, () => {}, changed)
}
