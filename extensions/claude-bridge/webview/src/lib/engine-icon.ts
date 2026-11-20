/** The icon of each agent: the engine-switch dividers in the chat and the
 *  amounts of the cost chip use the same one, so a sum reads as its agent's. */
export const ENGINE_ICON = { claude: 'fas fa-gem', codex: 'fas fa-robot' } as const

export function engineIcon(engine: unknown): string {
  return engine === 'codex' ? ENGINE_ICON.codex : ENGINE_ICON.claude
}
