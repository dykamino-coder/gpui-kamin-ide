// Shared look of the pills in the row under the chat input (limits, context,
// cost): one rounded chip per value, a thin meter coloured by level.

export const CHIP_STYLE = `
  display:inline-flex;align-items:center;gap:6px;min-width:0;flex:none;
  height:18px;padding:0 8px;border-radius:999px;box-sizing:border-box;
  background:color-mix(in srgb, var(--bg-overlay) 55%, transparent);
  border:1px solid color-mix(in srgb, var(--text-muted) 16%, transparent);
`

export const CHIP_LABEL_STYLE =
  'color:var(--text-muted);letter-spacing:0.04em;text-transform:uppercase;font-size:9px'

export const METER_TRACK_STYLE =
  'position:relative;width:34px;height:4px;flex:none;border-radius:2px;overflow:hidden;' +
  'background:color-mix(in srgb, var(--text-muted) 22%, transparent)'

export function meterFillStyle(percent: number, color: string): string {
  const fill = Math.max(0, Math.min(100, percent))
  return `position:absolute;left:0;top:0;bottom:0;width:${fill}%;border-radius:2px;background:${color};transition:width .4s ease`
}
