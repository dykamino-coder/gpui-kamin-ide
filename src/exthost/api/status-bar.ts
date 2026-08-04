// Host-side status-bar registry (B8). Extensions call
// `window.createStatusBarItem(...)`; the returned item's mutations (text /
// tooltip / command / show / hide / dispose) broadcast to the renderer's
// StatusBar component, which renders contributed items alongside the built-ins.
// Single host-wide instance, same fan-out-to-every-renderer caveat as Webviews.
type Broadcast = (channel: string, payload: unknown) => void

/** Serialisable item state pushed to the renderer (`kamin:statusBar:update`). */
export interface StatusBarItemState {
  id: string
  /** 1 = Left, 2 = Right (StatusBarAlignment). */
  alignment: number
  priority: number
  /** May contain `$(icon)` codicon tokens — the renderer expands them. */
  text: string
  tooltip?: string | undefined
  /** Command id to run on click (Command objects are reduced to their id). */
  command?: string | undefined
  /** Foreground colour — a literal CSS colour (`#3fb950`) or, for a ThemeColor,
   *  the resolved `var(--vscode-<id>)`. Undefined = default status-bar fg. */
  color?: string | undefined
  visible: boolean
}

/** VS Code's StatusBarItem.color is `string | ThemeColor`. Reduce to a CSS value:
 *  a literal string passes through; a ThemeColor `{id}` maps to its `--vscode-*` var. */
function resolveStatusColor(v: unknown): string | undefined {
  if (typeof v === "string") return v
  const id = (v as { id?: unknown } | null)?.id
  return typeof id === "string" ? `var(--vscode-${id.replace(/\./g, "-")})` : undefined
}

/** A vscode `Command` object or a bare command id. */
type CommandLike = string | { command?: string } | undefined

export class StatusBar {
  private seq = 0
  // Current visible items, so a renderer that connects AFTER activation can pull
  // a snapshot (broadcasts alone are lost if no renderer was listening yet).
  private readonly items = new Map<string, StatusBarItemState>()
  constructor(private readonly broadcast: Broadcast) {}

  /** Currently-VISIBLE items — pulled by the renderer on (re)connect. Hidden
   *  items stay in the Map (VS Code's `hide()` preserves props) but are filtered
   *  out of the snapshot. */
  snapshot(): StatusBarItemState[] { return [...this.items.values()].filter((i) => i.visible) }

  createItem(extId: string, alignment: number, priority: number): StatusBarItemHandle {
    const id = `${extId}.statusbar.${String(++this.seq)}`
    const state: StatusBarItemState = { id, alignment, priority, text: "", visible: false }
    // Arrows capture `this` lexically — the object-literal methods below can't.
    // sync keeps the Map current even while hidden, so a later show() re-emits
    // the latest text/tooltip/command.
    // VS Code's StatusBarItem.tooltip is `string | MarkdownString | undefined`.
    // Keep the raw value so a get round-trips the object the extension set (e.g.
    // rust-analyzer does `item.tooltip = new MarkdownString(); item.tooltip.isTrusted = true`
    // — coercing to a string here returned undefined and crashed that `.isTrusted`).
    let rawTooltip: unknown
    let rawColor: unknown
    const sync = (): void => { this.items.set(id, { ...state }) }
    const emit = (): void => { sync(); this.broadcast("kamin:statusBar:update", { ...state }) }
    const push = (): void => { sync(); if (state.visible) this.broadcast("kamin:statusBar:update", { ...state }) }
    const hideView = (): void => { sync(); this.broadcast("kamin:statusBar:remove", { id }) }
    const disposeItem = (): void => { this.items.delete(id); this.broadcast("kamin:statusBar:remove", { id }) }
    return {
      id,
      alignment,
      priority,
      name: undefined,
      get color() { return rawColor },
      set color(v: unknown) { rawColor = v; state.color = resolveStatusColor(v); push() },
      backgroundColor: undefined,
      accessibilityInformation: undefined,
      get text() { return state.text },
      set text(v: string) { state.text = v; push() },
      get tooltip() { return rawTooltip as string | undefined },
      set tooltip(v: string | undefined) {
        rawTooltip = v
        // The renderer state carries a plain string; a MarkdownString contributes its `.value`.
        const md = v as { value?: unknown } | null
        state.tooltip = typeof v === "string" ? v : typeof md?.value === "string" ? md.value : undefined
        push()
      },
      get command() { return state.command },
      set command(v: CommandLike) { state.command = typeof v === "string" ? v : v?.command; push() },
      show() { state.visible = true; emit() },
      hide() { state.visible = false; hideView() }, // keep props (VS Code semantics)
      dispose() { state.visible = false; disposeItem() },
    }
  }

  /** `window.setStatusBarMessage(text, hideAfterMs?)` — a transient left item. */
  setStatusBarMessage(text: string, hideAfterMs?: number): { dispose: () => void } {
    const item = this.createItem("workbench", 1, Number.MAX_SAFE_INTEGER)
    item.text = text
    item.show()
    if (typeof hideAfterMs === "number" && hideAfterMs > 0) setTimeout(() => { item.dispose() }, hideAfterMs)
    return { dispose: () => { item.dispose() } }
  }
}

export interface StatusBarItemHandle {
  readonly id: string
  alignment: number
  priority: number
  name: string | undefined
  color: unknown
  backgroundColor: unknown
  accessibilityInformation: unknown
  text: string
  tooltip: string | undefined
  /** Settable as a command id or Command object; stored + read back as the id. */
  command: CommandLike
  show(): void
  hide(): void
  dispose(): void
}
