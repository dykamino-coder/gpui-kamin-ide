import { CONSOLE_SCROLLBACK, CONSOLE_OVERVIEW_RULER, configureTerminalInteraction } from './interaction'
import { configureTerminalClipboard } from './clipboard'
import type { ClipboardOptions } from './clipboard'
import type { ConsoleInteractionTerminal, ConsoleClipboardTerminal } from './types'
export { createOsc52ClipboardProvider } from './clipboard'

/** Geometry and input policy shared by the CEF console and server dashboard. */
export const CONSOLE_OPTIONS = {
  scrollback: CONSOLE_SCROLLBACK,
  overviewRuler: CONSOLE_OVERVIEW_RULER,
  allowProposedApi: true,
  macOptionClickForcesSelection: true,
}

export function configureConsole(
  terminal: ConsoleInteractionTerminal & ConsoleClipboardTerminal,
  container: HTMLElement,
  clipboard: ClipboardOptions = {},
): () => void {
  const disposeInteraction = configureTerminalInteraction(terminal, container)
  const disposeClipboard = configureTerminalClipboard(terminal, container, clipboard)
  return () => {
    disposeClipboard()
    disposeInteraction()
  }
}
