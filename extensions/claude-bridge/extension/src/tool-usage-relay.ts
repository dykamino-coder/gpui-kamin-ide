/** Bridge Tools-panel action, shared by native and browser hosts. */
export function relayToolUsage(name: unknown, broadcast: (channel: string, name: string) => unknown): void {
  if (typeof name === 'string') broadcast('toolusage-open', name)
}
