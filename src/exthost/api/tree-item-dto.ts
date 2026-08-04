// TreeItem → TreeNodeDto mapping (extracted from tree-views.ts).
//
// Everything here is PURE and defensive: a TreeItem is whatever the extension
// set on it (plain properties, no class contract), so each field is read as
// `unknown` and narrowed. The registry owns handles + provider lifecycle; this
// module only turns one item into the flat shape the renderer can consume.
import type { TreeNodeDto } from "../../api/types.js"

/** Read a possibly-async TreeItem field set. Kept loose — extensions set fields
 *  as plain properties on the TreeItem instance. */
export interface TreeItemLike {
  label?: unknown
  id?: unknown
  description?: unknown
  tooltip?: unknown
  collapsibleState?: unknown
  iconPath?: unknown
  command?: unknown
  contextValue?: unknown
  resourceUri?: unknown
  checkboxState?: unknown
}

/** Narrow an arbitrary TreeItem field to a string. Exported because the registry
 *  reads `TreeItem.id` (the stable-handle source) through the same guard. */
export const asString = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined)

/** label is `string | TreeItemLabel{label}`; tooltip is `string | MarkdownString{value}`. */
function readText(v: unknown): string | undefined {
  if (typeof v === "string") return v
  if (v && typeof v === "object" && typeof (v as { value?: unknown }).value === "string") return (v as { value: string }).value
  if (v && typeof v === "object" && typeof (v as { label?: unknown }).label === "string") return (v as { label: string }).label
  return undefined
}

/** A Uri-like (fsPath/path) or string → an fs path string. */
function resourceString(v: unknown): string | undefined {
  if (typeof v === "string") return v
  if (v && typeof v === "object") {
    const o = v as { fsPath?: unknown; path?: unknown }
    if (typeof o.fsPath === "string") return o.fsPath
    if (typeof o.path === "string") return o.path
  }
  return undefined
}

/** iconPath → a codicon id (ThemeIcon: `{id}` with no Uri scheme), else treat as
 *  a resource (Uri / {light,dark} / string path) for file-icon resolution. */
function readIcon(iconPath: unknown): { codicon?: string; resourceUri?: string } {
  if (!iconPath) return {}
  if (typeof iconPath === "object" && "id" in iconPath && !("scheme" in iconPath) && !("fsPath" in iconPath)) {
    const id = (iconPath).id
    return typeof id === "string" ? { codicon: id } : {}
  }
  const uri = resourceString(iconPath) ?? resourceString((iconPath as { dark?: unknown }).dark) ?? resourceString((iconPath as { light?: unknown }).light)
  return uri ? { resourceUri: uri } : {}
}

function readCommand(v: unknown): TreeNodeDto["command"] {
  if (!v || typeof v !== "object") return undefined
  const c = v as { command?: unknown; title?: unknown; arguments?: unknown }
  if (typeof c.command !== "string") return undefined
  const out: NonNullable<TreeNodeDto["command"]> = { command: c.command }
  if (typeof c.title === "string") out.title = c.title
  if (Array.isArray(c.arguments)) out.arguments = c.arguments
  return out
}

/** checkboxState is `TreeItemCheckboxState` (a number) or `{ state, tooltip? }`. */
function readCheckbox(v: unknown): { state: number; tooltip?: string } | undefined {
  if (typeof v === "number") return { state: v }
  if (v && typeof v === "object" && typeof (v as { state?: unknown }).state === "number") {
    const o = v as { state: number; tooltip?: unknown }
    const tip = asString(o.tooltip)
    return { state: o.state, ...(tip !== undefined ? { tooltip: tip } : {}) }
  }
  return undefined
}

export function toDto(item: TreeItemLike, handle: string): TreeNodeDto {
  const icon = readIcon(item.iconPath)
  const resourceUri = resourceString(item.resourceUri)
  const dto: TreeNodeDto = {
    handle,
    label: readText(item.label) ?? (resourceUri ? resourceUri.split(/[\\/]/).pop() ?? "" : ""),
    collapsibleState: typeof item.collapsibleState === "number" ? item.collapsibleState : 0,
  }
  const description = asString(item.description)
  const tooltip = readText(item.tooltip)
  const command = readCommand(item.command)
  const contextValue = asString(item.contextValue)
  const resource = resourceUri ?? icon.resourceUri
  if (description) dto.description = description
  if (tooltip) dto.tooltip = tooltip
  if (icon.codicon) dto.codicon = icon.codicon
  if (resource) dto.resourceUri = resource
  if (command) dto.command = command
  if (contextValue) dto.contextValue = contextValue
  const checkbox = readCheckbox(item.checkboxState)
  if (checkbox) {
    dto.checkboxState = checkbox.state
    if (checkbox.tooltip) dto.checkboxTooltip = checkbox.tooltip
  }
  return dto
}
