// Real `vscode.window.createQuickPick()` / `createInputBox()` — the stateful
// object variants of showQuickPick/showInputBox. The extension configures the
// object (items/value/placeholder) then calls `.show()`; WITHOUT a real impl
// these were inert objects whose `onDidAccept` never fired, so any multi-step
// flow that awaits acceptance (auth sign-in, GitLens pickers, python interpreter
// selection) HUNG forever — strictly worse than returning undefined.
//
// We back `.show()` with the same host modal the simple showQuickPick/
// showInputBox pickers use, and fire accept/change/hide from its result. Full
// live filtering (onDidChangeValue per keystroke) isn't carried across the WS —
// the modal returns the final choice — but the object now completes instead of
// hanging, which is what the common flows need.
import type { QuickPickItemDto } from "../../api/types.js"
import type { NsHooks } from "./ns-builders.js"
import { EventEmitter, noopEvent } from "./shared.js"

interface QuickItem { label: string; description?: string; detail?: string; picked?: boolean; kind?: number; alwaysShow?: boolean }

export function makeInputBox(h: NsHooks): Record<string, unknown> {
  const onAccept = new EventEmitter<void>()
  const onChange = new EventEmitter<string>()
  const onHide = new EventEmitter<void>()
  const box = {
    value: "", placeholder: "", prompt: "", title: "", password: false,
    busy: false, enabled: true, ignoreFocusOut: false,
    step: undefined as number | undefined, totalSteps: undefined as number | undefined,
    buttons: [] as unknown[], valueSelection: undefined as [number, number] | undefined,
    show(): void {
      void h.showInputBox({ prompt: box.prompt || box.title, placeHolder: box.placeholder, value: box.value })
        .then((v) => {
          if (v === undefined) { onHide.fire(); return } // dismissed
          box.value = v
          onChange.fire(v)
          onAccept.fire()
        })
    },
    hide(): void { onHide.fire() },
    dispose(): void { onAccept.dispose(); onChange.dispose(); onHide.dispose() },
    onDidAccept: onAccept.event,
    onDidChangeValue: onChange.event,
    onDidHide: onHide.event,
    onDidTriggerButton: noopEvent,
  }
  return box
}

export function makeQuickPick(h: NsHooks): Record<string, unknown> {
  const onAccept = new EventEmitter<void>()
  const onChangeSel = new EventEmitter<readonly QuickItem[]>()
  const onChangeActive = new EventEmitter<readonly QuickItem[]>()
  const onChangeValue = new EventEmitter<string>()
  const onHide = new EventEmitter<void>()
  const qp = {
    value: "", placeholder: "", title: "",
    items: [] as readonly QuickItem[],
    activeItems: [] as readonly QuickItem[],
    selectedItems: [] as readonly QuickItem[],
    canSelectMany: false, busy: false, enabled: true, ignoreFocusOut: false,
    matchOnDescription: false, matchOnDetail: false,
    step: undefined as number | undefined, totalSteps: undefined as number | undefined,
    buttons: [] as unknown[],
    show(): void {
      const dto: QuickPickItemDto[] = qp.items.map((it) => ({
        label: it.label, description: it.description, detail: it.detail,
        kind: it.kind, picked: it.picked, alwaysShow: it.alwaysShow,
      }))
      void h.showQuickPick(dto, {
        canPickMany: qp.canSelectMany, placeHolder: qp.placeholder,
        title: qp.title, matchOnDescription: qp.matchOnDescription,
        matchOnDetail: qp.matchOnDetail, ignoreFocusOut: qp.ignoreFocusOut,
      }).then((idxs) => {
        if (!idxs || idxs.length === 0) { onHide.fire(); return } // dismissed
        const chosen = idxs.map((i) => qp.items[i]).filter(Boolean) as QuickItem[]
        qp.selectedItems = chosen
        qp.activeItems = chosen
        onChangeSel.fire(chosen)
        onAccept.fire()
      })
    },
    hide(): void { onHide.fire() },
    dispose(): void {
      onAccept.dispose(); onChangeSel.dispose(); onChangeActive.dispose()
      onChangeValue.dispose(); onHide.dispose()
    },
    onDidAccept: onAccept.event,
    onDidChangeSelection: onChangeSel.event,
    onDidChangeActive: onChangeActive.event,
    onDidChangeValue: onChangeValue.event,
    onDidHide: onHide.event,
    onDidTriggerButton: noopEvent,
    onDidTriggerItemButton: noopEvent,
  }
  return qp
}
