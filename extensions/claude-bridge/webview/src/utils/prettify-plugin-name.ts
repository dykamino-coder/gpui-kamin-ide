export function prettifyPluginName(name: string): string {
  return name
    .replace(/[-_]/g, ' ')
    .replace(/^\w/, c => c.toUpperCase())
}
